//! Persistent, opt-in cache of file hashes, so a duplicate search doesn't
//! have to re-read every file's content on a later scan of the same drive.
//!
//! Stored under LocalAppData/DiscReveal/Cache. Entirely opt-in: no database
//! or cache directory is created by inspecting cache statistics. Writes happen
//! only when `duplicates.rs`'s `find_duplicates` is called with `use_cache:
//! true`. Several `duplicates.rs` worker threads open their own connection to
//! this same file concurrently, which is why `open` turns on WAL journaling
//! and a busy timeout: that's the standard way to let a handful of
//! short-lived SQLite connections share one file without `SQLITE_BUSY`
//! errors under normal (non-pathological) contention.

use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

const MAX_ENTRIES: usize = 100_000;
const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;
const CACHE_TTL_SECONDS: u64 = 30 * 86_400;

const CACHE_FILE_NAME: &str = "discreveal_cache.sqlite3";
static ACCESS: RwLock<()> = RwLock::new(());

fn cache_file_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("DiscReveal").join("Cache"))
        .unwrap_or_else(|| crate::data_dir().join("Cache"))
        .join(CACHE_FILE_NAME)
}

/// Prevent clearing a cache while this process has a cached scan in flight.
pub(crate) fn scan_access() -> RwLockReadGuard<'static, ()> {
    ACCESS.read().unwrap_or_else(|error| error.into_inner())
}

/// Opens (creating if necessary) the cache database and its one table.
pub(crate) fn open() -> rusqlite::Result<Connection> {
    open_at(&cache_file_path())
}

fn open_at(path: &Path) -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let conn = Connection::open(path)?;
    // Caching is opportunistic: a contended cache must not hold up cancellation for seconds.
    conn.pragma_update(None, "busy_timeout", 100)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_hashes (
            path TEXT PRIMARY KEY,
            size INTEGER NOT NULL,
            modified INTEGER NOT NULL,
            hash INTEGER NOT NULL,
            last_seen INTEGER NOT NULL DEFAULT 0,
            sample_version INTEGER NOT NULL DEFAULT 0
        )",
        (),
    )?;
    // Old 256 KiB prefix samples are never interchangeable with multi-position samples.
    let has_version = conn
        .prepare("PRAGMA table_info(file_hashes)")?
        .query_map((), |row| row.get::<_, String>(1))?
        .flatten()
        .any(|name| name == "sample_version");
    if !has_version {
        let _ = conn.execute(
            "ALTER TABLE file_hashes ADD COLUMN sample_version INTEGER NOT NULL DEFAULT 0",
            (),
        );
    }
    let has_seen = conn
        .prepare("PRAGMA table_info(file_hashes)")?
        .query_map((), |row| row.get::<_, String>(1))?
        .flatten()
        .any(|name| name == "last_seen");
    if !has_seen {
        conn.execute(
            "ALTER TABLE file_hashes ADD COLUMN last_seen INTEGER NOT NULL DEFAULT 0",
            (),
        )?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS file_hashes_last_seen ON file_hashes(last_seen)",
        (),
    )?;
    let page_size: u64 = conn.query_row("PRAGMA page_size", (), |row| row.get(0))?;
    conn.pragma_update(
        None,
        "max_page_count",
        (MAX_CACHE_BYTES / page_size.max(1)).max(1),
    )?;
    Ok(conn)
}

/// Returns the cached hash for `path`, but only if the cached `size` and
/// `modified` still match what was just observed on disk — a changed file
/// naturally misses here rather than needing separate invalidation logic.
pub(crate) fn lookup(conn: &Connection, path: &str, size: u64, modified: u64) -> Option<u64> {
    conn.prepare_cached("SELECT hash FROM file_hashes WHERE path = ?1 AND size = ?2 AND modified = ?3 AND sample_version = 2").ok()?.query_row(
        (path, size, modified),
        |row| row.get::<_, i64>(0).map(|hash| hash as u64),
    )
    .ok()
}

/// Stores all 64 hash bits in a signed SQLite INTEGER. Positive legacy values remain valid.
#[cfg(test)]
pub(crate) fn store(conn: &Connection, path: &str, size: u64, modified: u64, hash: u64) {
    let _ = store_batch(
        conn,
        &[CacheEntry {
            path: path.to_owned(),
            size,
            modified,
            hash,
        }],
    );
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    exists: bool,
    entry_count: u64,
    file_size_bytes: u64,
}

/// Reports the cache's current size, or all-zero/`exists: false` if it
/// hasn't been created yet (not an error — that's the normal state for
/// anyone who has never opted in).
#[tauri::command]
pub fn hash_cache_stats() -> AppResult<CacheStats> {
    let _access = scan_access();
    Ok(stats_at(&cache_file_path()))
}

pub(crate) struct CacheEntry {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub hash: u64,
}

/// Commit a bounded batch rather than one WAL transaction for every candidate.
pub(crate) fn store_batch(conn: &Connection, entries: &[CacheEntry]) -> rusqlite::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let transaction = conn.unchecked_transaction()?;
    {
        let mut statement = transaction.prepare_cached("INSERT OR REPLACE INTO file_hashes (path,size,modified,hash,sample_version,last_seen) VALUES (?1,?2,?3,?4,2,?5)")?;
        for entry in entries {
            statement.execute((
                &entry.path,
                entry.size,
                entry.modified,
                entry.hash as i64,
                now_seconds(),
            ))?;
        }
    }
    prune_entries(&transaction, MAX_ENTRIES, now_seconds())?;
    transaction.commit()
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn prune_entries(conn: &Connection, max_entries: usize, now: u64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM file_hashes WHERE last_seen < ?1",
        [now.saturating_sub(CACHE_TTL_SECONDS)],
    )?;
    conn.execute("DELETE FROM file_hashes WHERE rowid IN (SELECT rowid FROM file_hashes ORDER BY last_seen, rowid LIMIT MAX(0,(SELECT COUNT(*) FROM file_hashes)-?1))", [max_entries as u64])?;
    Ok(())
}

/// Only invoked after explicit opt-in; stats never creates or maintains a DB.
pub(crate) fn prepare_scan() -> rusqlite::Result<()> {
    let conn = open()?;
    prune_entries(&conn, MAX_ENTRIES, now_seconds())
}

fn stats_at(path: &Path) -> CacheStats {
    if !path.is_file() {
        return CacheStats {
            exists: false,
            entry_count: 0,
            file_size_bytes: 0,
        };
    }
    let entry_count = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM file_hashes", (), |row| {
                row.get::<_, i64>(0)
            })
        })
        .map(|count| count.max(0) as u64)
        .unwrap_or(0);
    let file_size_bytes = [
        path.to_path_buf(),
        append_suffix(path, "-wal"),
        append_suffix(path, "-shm"),
    ]
    .iter()
    .filter_map(|file| std::fs::metadata(file).ok())
    .map(|metadata| metadata.len())
    .sum();
    CacheStats {
        exists: true,
        entry_count,
        file_size_bytes,
    }
}

/// Checkpoint after worker connections close. SQLite truncates/removes sidecars
/// when safe; another process's active transaction is respected, never unlinked.
pub(crate) fn finish_scan() {
    let Ok(_access) = ACCESS.try_write() else {
        return;
    };
    let path = cache_file_path();
    let _ = checkpoint_at(&path);
    if let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE) {
        let _ = conn.busy_timeout(std::time::Duration::from_millis(100));
        if let Ok(size) = std::fs::metadata(&path).map(|meta| meta.len())
            && size > MAX_CACHE_BYTES
        {
            let _ = conn.execute_batch("VACUUM");
        }
    }
}

fn checkpoint_at(path: &Path) -> rusqlite::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(std::time::Duration::from_millis(100))?;
    let _: (i64, i64, i64) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", (), |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    Ok(())
}

/// Deletes the cache file and its WAL/shared-memory sidecars, if present.
/// Missing files are not an error — clearing an already-empty cache succeeds.
#[tauri::command]
pub fn clear_hash_cache() -> AppResult<()> {
    let _access = ACCESS
        .try_write()
        .map_err(|_| AppError::new("cacheInUse"))?;
    let path = cache_file_path();
    clear_at(&path)
}

fn clear_at(path: &Path) -> AppResult<()> {
    for candidate in [
        path.to_path_buf(),
        append_suffix(path, "-wal"),
        append_suffix(path, "-shm"),
    ] {
        if let Err(error) = std::fs::remove_file(&candidate)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(AppError::with_detail(
                "cacheClearFailed",
                format!("{} ({error})", candidate.display()),
            ));
        }
    }
    Ok(())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_eviction_respects_both_ttl_and_a_global_entry_budget() {
        let path = test_path();
        let conn = open_at(&path).unwrap();
        let now = now_seconds();
        for (name, stamp) in [
            ("expired", now - CACHE_TTL_SECONDS - 1),
            ("older", now - 5),
            ("newer", now - 1),
        ] {
            conn.execute("INSERT INTO file_hashes(path,size,modified,hash,sample_version,last_seen) VALUES (?1,1,1,1,2,?2)", (name, stamp)).unwrap();
        }
        prune_entries(&conn, 1, now).unwrap();
        assert_eq!(lookup(&conn, "expired", 1, 1), None);
        assert_eq!(lookup(&conn, "older", 1, 1), None);
        assert_eq!(lookup(&conn, "newer", 1, 1), Some(1));
        drop(conn);
        clear_at(&path).unwrap();
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn legacy_prefix_samples_are_invalidated_without_losing_the_cache_file() {
        let path = test_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let legacy = Connection::open(&path).unwrap();
        legacy.execute_batch("CREATE TABLE file_hashes (path TEXT PRIMARY KEY,size INTEGER,modified INTEGER,hash INTEGER); INSERT INTO file_hashes VALUES ('old',10,20,42)").unwrap();
        drop(legacy);
        let conn = open_at(&path).unwrap();
        assert_eq!(lookup(&conn, "old", 10, 20), None);
        store(&conn, "old", 10, 20, 77);
        assert_eq!(lookup(&conn, "old", 10, 20), Some(77));
        drop(conn);
        clear_at(&path).unwrap();
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn cache_batch_commits_together_and_rolls_back_on_failure() {
        let path = test_path();
        let conn = open_at(&path).unwrap();
        let entries = vec![
            CacheEntry {
                path: "good".into(),
                size: 10,
                modified: 20,
                hash: u64::MAX,
            },
            CacheEntry {
                path: "bad".into(),
                size: 10,
                modified: 20,
                hash: 7,
            },
        ];
        conn.execute_batch("CREATE TRIGGER reject_bad BEFORE INSERT ON file_hashes WHEN NEW.path = 'bad' BEGIN SELECT RAISE(ABORT,'test rejection'); END;").unwrap();
        assert!(store_batch(&conn, &entries).is_err());
        assert_eq!(lookup(&conn, "good", 10, 20), None);
        conn.execute_batch("DROP TRIGGER reject_bad").unwrap();
        store_batch(&conn, &entries).unwrap();
        assert_eq!(lookup(&conn, "good", 10, 20), Some(u64::MAX));
        assert_eq!(lookup(&conn, "bad", 10, 20), Some(7));
        drop(conn);
        clear_at(&path).unwrap();
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    fn test_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("discreveal-cache-{}", rand::random::<u64>()))
            .join(CACHE_FILE_NAME)
    }

    #[test]
    fn stats_do_not_create_a_cache_without_opt_in() {
        let path = test_path();
        assert!(!stats_at(&path).exists);
        assert!(!path.parent().unwrap().exists());
        checkpoint_at(&path).unwrap();
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn stats_include_sidecars_and_checkpoint_keeps_cached_hashes() {
        let path = test_path();
        let conn = open_at(&path).unwrap();
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        store(&conn, "file", 10, 20, u64::MAX);
        let expected: u64 = [
            path.clone(),
            append_suffix(&path, "-wal"),
            append_suffix(&path, "-shm"),
        ]
        .iter()
        .map(|file| std::fs::metadata(file).unwrap().len())
        .sum();
        let stats = stats_at(&path);
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.file_size_bytes, expected);
        assert!(stats.file_size_bytes > std::fs::metadata(&path).unwrap().len());
        drop(conn);
        checkpoint_at(&path).unwrap();
        let conn = open_at(&path).unwrap();
        assert_eq!(lookup(&conn, "file", 10, 20), Some(u64::MAX));
        drop(conn);
        assert!(!append_suffix(&path, "-wal").exists());
        assert!(!append_suffix(&path, "-shm").exists());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn clearing_is_refused_while_a_cached_scan_is_active() {
        let _access = scan_access();
        assert_eq!(clear_hash_cache().unwrap_err().code, "cacheInUse");
    }

    #[test]
    fn checkpoint_respects_an_active_reader_and_preserves_pending_writes() {
        let path = test_path();
        let writer = open_at(&path).unwrap();
        store(&writer, "file", 10, 20, 1);
        let reader = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        reader.execute_batch("BEGIN").unwrap();
        assert_eq!(lookup(&reader, "file", 10, 20), Some(1));
        store(&writer, "file", 10, 20, 2);
        checkpoint_at(&path).unwrap();
        assert_eq!(lookup(&reader, "file", 10, 20), Some(1));
        assert!(
            std::fs::metadata(append_suffix(&path, "-wal"))
                .unwrap()
                .len()
                > 0
        );
        reader.execute_batch("COMMIT").unwrap();
        drop(reader);
        checkpoint_at(&path).unwrap();
        assert_eq!(lookup(&writer, "file", 10, 20), Some(2));
        drop(writer);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn all_hash_bits_round_trip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE file_hashes (path TEXT PRIMARY KEY, size INTEGER, modified INTEGER, hash INTEGER, last_seen INTEGER NOT NULL DEFAULT 0, sample_version INTEGER NOT NULL DEFAULT 0)", ()).unwrap();
        for hash in [0, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
            store(&conn, "file", 10, 20, hash);
            assert_eq!(lookup(&conn, "file", 10, 20), Some(hash));
        }
    }

    #[test]
    fn a_stored_hash_is_found_only_with_matching_size_and_modified() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE file_hashes (path TEXT PRIMARY KEY, size INTEGER NOT NULL, modified INTEGER NOT NULL, hash INTEGER NOT NULL, last_seen INTEGER NOT NULL DEFAULT 0, sample_version INTEGER NOT NULL DEFAULT 0)",
            (),
        )
        .unwrap();
        store(&conn, r"C:\a\b.txt", 100, 1_700_000_000, 42);

        assert_eq!(lookup(&conn, r"C:\a\b.txt", 100, 1_700_000_000), Some(42));
        assert_eq!(lookup(&conn, r"C:\a\b.txt", 101, 1_700_000_000), None); // size changed
        assert_eq!(lookup(&conn, r"C:\a\b.txt", 100, 1_700_000_001), None); // modified changed
        assert_eq!(lookup(&conn, r"C:\other.txt", 100, 1_700_000_000), None);
    }

    #[test]
    fn clearing_a_missing_cache_succeeds() {
        // Nothing has opted in yet on a fresh install — clearing must be a harmless no-op, not
        // an error surfaced to the user for a cache that was never created.
        assert!(clear_at(&test_path()).is_ok());
    }

    #[test]
    fn clear_hash_cache_removes_an_existing_file() {
        let path = test_path();
        drop(open_at(&path).unwrap()); // creates the file
        assert!(path.exists());
        assert!(clear_at(&path).is_ok());
        assert!(!path.exists());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn storing_again_replaces_the_previous_value() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE file_hashes (path TEXT PRIMARY KEY, size INTEGER NOT NULL, modified INTEGER NOT NULL, hash INTEGER NOT NULL, last_seen INTEGER NOT NULL DEFAULT 0, sample_version INTEGER NOT NULL DEFAULT 0)",
            (),
        )
        .unwrap();
        store(&conn, r"C:\a.txt", 10, 1, 1);
        store(&conn, r"C:\a.txt", 20, 2, 2);

        assert_eq!(lookup(&conn, r"C:\a.txt", 10, 1), None);
        assert_eq!(lookup(&conn, r"C:\a.txt", 20, 2), Some(2));
    }
}
