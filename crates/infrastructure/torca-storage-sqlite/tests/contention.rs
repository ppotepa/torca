use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use torca_storage_sqlite::{DatabaseKey, SqlCipherBackend, StorageBackend, StorageKernel};

fn temporary_database_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("torca-contention-{}-{stamp}.db", std::process::id()))
}

fn open_bootstrapped(path: &Path) -> SqlCipherBackend {
    let key = DatabaseKey::new([0x5a; 32]);
    let backend = SqlCipherBackend::open(path, &key).expect("open SQLCipher backend");
    let mut kernel = StorageKernel::new(backend);
    kernel.bootstrap().expect("bootstrap schema");
    kernel.into_backend()
}

fn remove_database_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn second_writer_waits_for_busy_timeout_instead_of_failing_immediately() {
    let path = temporary_database_path();
    let mut first = open_bootstrapped(&path);
    let mut second = open_bootstrapped(&path);

    first.begin().expect("first writer lock");
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        ready_tx.send(()).expect("signal writer start");
        let started = Instant::now();
        let result = second.begin();
        let waited = started.elapsed();
        if result.is_ok() {
            second.rollback().expect("release second writer");
        }
        (result, waited)
    });

    ready_rx.recv().expect("second writer started");
    thread::sleep(Duration::from_millis(100));
    first.rollback().expect("release first writer");

    let (result, waited) = worker.join().expect("second writer thread");
    assert!(result.is_ok(), "busy_timeout should bridge short writer contention: {result:?}");
    assert!(
        waited >= Duration::from_millis(50),
        "second writer unexpectedly bypassed the held write lock: waited {waited:?}"
    );
    assert!(
        waited < Duration::from_secs(5),
        "short contention should finish before the configured busy timeout: waited {waited:?}"
    );

    drop(first);
    remove_database_files(&path);
}
