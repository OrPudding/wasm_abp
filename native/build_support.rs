use std::hash::{Hash, Hasher};
use std::path::Path;

fn hash_tree(path: &Path, hasher: &mut impl Hasher) {
    if path.is_dir() {
        let mut paths = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            hash_tree(&path, hasher);
        }
    } else {
        println!("cargo:rerun-if-changed={}", path.display());
        std::fs::read(path).unwrap().hash(hasher);
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = if manifest.join("src").is_dir() {
        manifest
    } else {
        manifest.parent().unwrap().to_path_buf()
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for path in [
        "src",
        "Cargo.lock",
        "Cargo.toml",
        "vendor/wasmtime/src",
        "vendor/wasmtime/Cargo.toml",
        "patches",
        "build_support.rs",
    ] {
        hash_tree(&root.join(path), &mut hasher);
    }
    if let Ok(output) =
        std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
            .arg("-vV")
            .output()
    {
        output.stdout.hash(&mut hasher);
    }
    for name in [
        "TARGET",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE_RELEASE_LTO",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_CFG_TARGET_FEATURE",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        std::env::var(name).unwrap_or_default().hash(&mut hasher);
    }
    println!(
        "cargo:rustc-env=ORONBOX_RUNTIME_BUILD={:016x}",
        hasher.finish()
    );
}
