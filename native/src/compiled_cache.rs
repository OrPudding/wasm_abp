use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

static DIRECTORY: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);
const HEADER: &[u8] = b"OBWASM01";

fn directory() -> &'static RwLock<Option<PathBuf>> {
    DIRECTORY.get_or_init(|| RwLock::new(None))
}

pub fn configure(path: &str) -> Result<(), String> {
    let mut root = directory().write().unwrap();
    if path.is_empty() {
        *root = None;
        return Ok(());
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("Compilation cache path must be absolute".into());
    }
    *root = Some(path);
    Ok(())
}

pub struct CompilationCache<T> {
    objects: Mutex<HashMap<String, Arc<Mutex<Weak<T>>>>>,
    memory_hits: AtomicU64,
    disk_hits: AtomicU64,
    compilations: AtomicU64,
    disk_errors: AtomicU64,
}

impl<T> Default for CompilationCache<T> {
    fn default() -> Self {
        Self {
            objects: Mutex::new(HashMap::new()),
            memory_hits: AtomicU64::new(0),
            disk_hits: AtomicU64::new(0),
            compilations: AtomicU64::new(0),
            disk_errors: AtomicU64::new(0),
        }
    }
}

impl<T> CompilationCache<T> {
    pub fn acquire(
        &self,
        bytes: &[u8],
        environment: impl Hash,
        kind: &str,
        deserialize: impl Fn(&[u8]) -> Result<T, String>,
        compile: impl FnOnce() -> Result<T, String>,
        serialize: impl FnOnce(&T) -> Result<Vec<u8>, String>,
    ) -> Result<Arc<T>, String> {
        let source = format!("{:x}", Sha256::digest(bytes));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (env!("ORONBOX_RUNTIME_BUILD"), environment, kind).hash(&mut hasher);
        let fingerprint = format!("{:016x}", hasher.finish());
        let key = format!("{fingerprint}/{source}");
        let slot = {
            let mut objects = self.objects.lock().unwrap();
            objects.retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot
                        .try_lock()
                        .map_or(true, |object| object.strong_count() > 0)
            });
            objects
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(Weak::new())))
                .clone()
        };
        // A per-content lock merges concurrent requests without serializing other modules.
        let mut cached = slot.lock().unwrap();
        if let Some(object) = cached.upgrade() {
            self.memory_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(object);
        }
        let root = directory().read().unwrap();
        let path = root
            .as_ref()
            .map(|root| root.join(&fingerprint).join(format!("{source}.cwasm")));
        if let Some(path) = &path {
            match read_artifact(path).and_then(|bytes| deserialize(&bytes)) {
                Ok(object) => {
                    self.disk_hits.fetch_add(1, Ordering::Relaxed);
                    let object = Arc::new(object);
                    *cached = Arc::downgrade(&object);
                    return Ok(object);
                }
                Err(_) if path.exists() => {
                    self.disk_errors.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {}
            }
        }
        let object = Arc::new(compile()?);
        self.compilations.fetch_add(1, Ordering::Relaxed);
        if let Some(path) = &path {
            if serialize(&object)
                .and_then(|bytes| write_artifact(path, &bytes))
                .is_err()
            {
                self.disk_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
        *cached = Arc::downgrade(&object);
        Ok(object)
    }

    pub fn diagnostics(&self) -> serde_json::Value {
        let objects = self.objects.lock().unwrap();
        serde_json::json!({
            "activeCompiledObjects": objects.values().filter(|slot| slot.lock().unwrap().strong_count() > 0).count(),
            "memoryHits": self.memory_hits.load(Ordering::Relaxed),
            "diskHits": self.disk_hits.load(Ordering::Relaxed),
            "compilations": self.compilations.load(Ordering::Relaxed),
            "diskErrors": self.disk_errors.load(Ordering::Relaxed),
            "diskEnabled": directory().read().unwrap().is_some(),
        })
    }
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, String> {
    if std::fs::symlink_metadata(path)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Compilation cache contains a symbolic link".into());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let header_size = HEADER.len() + 32;
    if bytes.len() < header_size
        || &bytes[..HEADER.len()] != HEADER
        || Sha256::digest(&bytes[header_size..]).as_slice() != &bytes[HEADER.len()..header_size]
    {
        return Err("Compilation cache artifact is incomplete or corrupted".into());
    }
    Ok(bytes[header_size..].to_vec())
}

fn write_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension(format!(
        "{}.{}.part",
        std::process::id(),
        NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(HEADER)?;
        file.write_all(&Sha256::digest(bytes))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    let _ = std::fs::remove_file(temporary);
    result.map_err(|error| error.to_string())
}
