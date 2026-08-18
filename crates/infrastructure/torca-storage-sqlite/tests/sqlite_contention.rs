use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::params;
use torca_storage_sqlite::{DatabaseKey, SqlCipherBackend, StorageKernel};

static NEXT_DB: AtomicU64 = AtomicU64::new(1);

const SETUP_SQL: &str = include_str!("sql/contention_setup.sql");
const INSERT_SQL: &str = include_str!("sql/contention_insert.sql");
const COUNT_SQL: &str = include_str!("sql/contention_count.sql");
const BEGIN_SQL: &str = include_str!("sql/contention_begin.sql");
const COMMIT_SQL: &str = include_str!("sql/contention_commit.sql");

struct TestDatabase(PathBuf);

impl TestDatabase {
    fn new(label: &str) -> Self {
        let id = NEXT_DB.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!("torca-{label}-{}-{id}.db", std::process::id())))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        for path in [
            self.0.clone(),
            PathBuf::from(format!("{}-wal", self.0.display())),
            PathBuf::from(format!("{}-shm", self.0.display())),
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn open_configured(path: &Path) -> SqlCipherBackend {
    let key = DatabaseKey::new([0x5a; 32]);
    let backend = SqlCipherBackend::open(path, &key).expect("open SQLCipher database");
    let mut kernel = StorageKernel::new(backend);
    kernel.bootstrap().expect("bootstrap storage policy");
    kernel.into_backend()
}

#[test]
fn busy_timeout_waits_for_a_short_lived_writer_instead_of_failing_immediately() {
    let database = TestDatabase::new("contention-wait");
    let writer = open_configured(database.path());
    writer.connection().execute_batch(SETUP_SQL).expect("create contention table");
    let waiting_writer = open_configured(database.path());

    writer.connection().execute_batch(BEGIN_SQL).expect("hold write lock");
    let started = Instant::now();
    let waiter = thread::spawn(move || {
        waiting_writer
            .connection()
            .execute(INSERT_SQL, params![1_i64])
            .expect("writer waits for lock and succeeds");
        started.elapsed()
    });

    thread::sleep(Duration::from_millis(150));
    writer.connection().execute_batch(COMMIT_SQL).expect("release write lock");
    let waited = waiter.join().expect("writer thread");
    assert!(waited >= Duration::from_millis(100));
    assert!(waited < Duration::from_secs(5));

    let count: i64 = writer
        .connection()
        .query_row(COUNT_SQL, [], |row| row.get(0))
        .expect("count committed rows");
    assert_eq!(count, 1);
}

#[test]
fn several_sqlcipher_connections_can_commit_a_writer_burst_without_busy_errors() {
    const WRITERS: usize = 4;
    const WRITES_PER_WRITER: usize = 25;

    let database = TestDatabase::new("contention-burst");
    let verifier = open_configured(database.path());
    verifier.connection().execute_batch(SETUP_SQL).expect("create contention table");

    // Bootstrap every connection before the synchronized burst so this test
    // measures normal application writer contention, not migration startup.
    let connections = (0..WRITERS).map(|_| open_configured(database.path())).collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut workers = Vec::with_capacity(WRITERS);

    for (worker_index, connection) in connections.into_iter().enumerate() {
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            for index in 0..WRITES_PER_WRITER {
                let value = i64::try_from(worker_index * 10_000 + index).expect("test value");
                connection
                    .connection()
                    .execute(INSERT_SQL, params![value])
                    .expect("bounded contention write");
            }
        }));
    }

    for worker in workers {
        worker.join().expect("writer thread");
    }

    let count: i64 =
        verifier.connection().query_row(COUNT_SQL, [], |row| row.get(0)).expect("count burst rows");
    assert_eq!(count, i64::try_from(WRITERS * WRITES_PER_WRITER).expect("count fits"));
}
