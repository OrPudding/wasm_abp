#[path = "../src/compiled_cache.rs"]
mod compiled_cache;

use compiled_cache::{configure, CompilationCache};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

#[test]
fn content_cache_shares_active_code_persists_recovers_and_retries() {
    let root = std::env::temp_dir().join(format!("oronbox-cache-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    configure(root.to_str().unwrap()).unwrap();
    let cache = Arc::new(CompilationCache::<Vec<u8>>::default());
    let compilations = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));
    let workers = (0..8)
        .map(|_| {
            let cache = cache.clone();
            let compilations = compilations.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cache
                    .acquire(
                        b"source",
                        "runtime",
                        "core",
                        |bytes| Ok(bytes.to_vec()),
                        || {
                            compilations.fetch_add(1, Ordering::SeqCst);
                            Ok(vec![42])
                        },
                        |bytes| Ok(bytes.clone()),
                    )
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let objects = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(compilations.load(Ordering::SeqCst), 1);
    assert!(objects
        .iter()
        .all(|object| Arc::ptr_eq(object, &objects[0])));
    assert_eq!(cache.diagnostics()["activeCompiledObjects"], 1);
    drop(objects);
    assert_eq!(cache.diagnostics()["activeCompiledObjects"], 0);

    // A fresh cache has no in-memory state and must recover its code from disk.
    let restarted = CompilationCache::<Vec<u8>>::default();
    let object = restarted
        .acquire(
            b"source",
            "runtime",
            "core",
            |bytes| Ok(bytes.to_vec()),
            || Err("unexpected compilation".into()),
            |bytes| Ok(bytes.clone()),
        )
        .unwrap();
    assert_eq!(*object, vec![42]);
    assert_eq!(restarted.diagnostics()["diskHits"], 1);
    drop(object);
    let fingerprint = std::fs::read_dir(&root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let artifact = std::fs::read_dir(fingerprint)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&artifact, b"partial write").unwrap();
    let repaired = restarted
        .acquire(
            b"source",
            "runtime",
            "core",
            |bytes| Ok(bytes.to_vec()),
            || Ok(vec![43]),
            |bytes| Ok(bytes.clone()),
        )
        .unwrap();
    assert_eq!(*repaired, vec![43]);
    assert_eq!(restarted.diagnostics()["diskErrors"], 1);
    drop(repaired);
    let recovered = restarted
        .acquire(
            b"source",
            "runtime",
            "core",
            |bytes| Ok(bytes.to_vec()),
            || Err("repair did not persist".into()),
            |bytes| Ok(bytes.clone()),
        )
        .unwrap();
    assert_eq!(*recovered, vec![43]);
    drop(recovered);

    assert!(restarted
        .acquire(
            b"invalid",
            "runtime",
            "core",
            |_| unreachable!(),
            || Err("invalid source".into()),
            |bytes| Ok(bytes.clone())
        )
        .is_err());
    let retried = restarted
        .acquire(
            b"invalid",
            "runtime",
            "core",
            |bytes| Ok(bytes.to_vec()),
            || Ok(vec![44]),
            |bytes| Ok(bytes.clone()),
        )
        .unwrap();
    assert_eq!(*retried, vec![44]);
    let other_runtime = restarted
        .acquire(
            b"source",
            "other-runtime",
            "core",
            |_| unreachable!(),
            || Ok(vec![45]),
            |bytes| Ok(bytes.clone()),
        )
        .unwrap();
    assert_eq!(*other_runtime, vec![45]);

    configure("").unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(*retried, vec![44]);
    assert_eq!(*other_runtime, vec![45]);
}

#[test]
fn clearing_waits_for_an_inflight_disk_writer() {
    let root = std::env::temp_dir().join(format!("oronbox-cache-flight-{}", std::process::id()));
    // This test runs in its own process to keep the process-wide configuration isolated.
    if std::env::var_os("ORONBOX_CACHE_CLEAR_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "clearing_waits_for_an_inflight_disk_writer"])
            .env("ORONBOX_CACHE_CLEAR_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    configure(root.to_str().unwrap()).unwrap();
    let (entered, started) = std::sync::mpsc::channel();
    let (resume, resumed) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        CompilationCache::<Vec<u8>>::default()
            .acquire(
                b"source",
                "runtime",
                "core",
                |_| unreachable!(),
                || {
                    entered.send(()).unwrap();
                    resumed.recv().unwrap();
                    Ok(vec![42])
                },
                |bytes| Ok(bytes.clone()),
            )
            .unwrap()
    });
    started.recv().unwrap();
    let (finished, completion) = std::sync::mpsc::channel();
    let clearing = std::thread::spawn(move || {
        configure("").unwrap();
        finished.send(()).unwrap();
    });
    assert!(completion
        .recv_timeout(std::time::Duration::from_millis(30))
        .is_err());
    resume.send(()).unwrap();
    let object = worker.join().unwrap();
    clearing.join().unwrap();
    completion.recv().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(*object, vec![42]);
}
