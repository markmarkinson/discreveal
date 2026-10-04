//! Finds files with identical content across one or more local drives.
//!
//! Files are enumerated with cached directory metadata and early size filtering, and
//! grouped first by size, then by small start/middle/end content samples.
//! Large collision buckets switch adaptively to full-content fingerprints
//! instead of repeatedly comparing unrelated files. Matches are still confirmed byte-for-byte before
//! being reported as duplicates: a hash collision (or two files that merely
//! share a prefix) must never turn into a false "these are the same file",
//! especially since the frontend offers to delete everything but one copy of
//! a reported group.
//!
//! Independent of the main scanner: a search can be started for any local
//! drive without one having been scanned first, validated the same way
//! `start_scan` validates its own root (`is_local_drive_path`), and runs as
//! a cancellable background job reporting progress through the
//! `duplicates:*` events, mirroring the `scan:*` events.

use crate::diagnostics::{Diagnostics, Snapshot as DiagnosticSnapshot};
use crate::error::{AppError, AppResult};
use crate::scan::{MAX_DEPTH, is_local_drive_path};
use crate::{cache, guard};
use rayon::prelude::*;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

const HASH_CHUNK_BYTES: usize = 8 * 1024;
#[cfg(test)]
const HASH_SAMPLE_BYTES: usize = 3 * HASH_CHUNK_BYTES;
const COMPARE_CHUNK_BYTES: usize = 256 * 1024;
const MAX_CANDIDATES: u64 = 250_000;
const MAX_CANDIDATE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REPRESENTATIVE_BYTES: u64 = 2 * 1024 * 1024;
/// Minimum time between two `duplicates:progress` events, so reporting never
/// slows the actual walk/hash work down.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);

const EVENT_PROGRESS: &str = "duplicates:progress";
const EVENT_GROUP: &str = "duplicates:group";
const EVENT_DONE: &str = "duplicates:done";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    path: String,
    name: String,
    modified: u64,
    cleanup_eligible: bool,
    cleanup_reason: Option<&'static str>,
    /// None means allocation could not be measured, never a logical-size guess.
    allocated_bytes: Option<u64>,
    reclaimable_bytes: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    size: u64,
    sample_hash: String,
    files: Vec<DuplicateFile>,
}

/// A file found during the walk, before it's known whether anything else
/// shares its size (and so whether it's worth hashing at all). Carries its
/// own `size` (duplicated from the `by_size` map key it started in) so it
/// can be freely moved into a flat, per-worker-thread chunk during hashing
/// and still know which `DuplicateGroup` it belongs to afterwards.
#[derive(Clone)]
struct Candidate {
    path: PathBuf,
    name: String,
    size: u64,
    modified: u64,
    cache_stamp: u64,
    sample_hash: Option<u64>,
    file_id: Option<(u64, [u8; 16])>,
    storage: Option<crate::storage::Measurement>,
}

mod policy;
pub use policy::DuplicateFilter;
#[cfg(test)]
use policy::TypeMode;
fn cleanup_eligible(path: &Path) -> bool {
    let plain = guard::plain_path(path.to_path_buf());
    policy::cleanup_eligible(&plain, guard::protection_of(&plain).is_some())
}

/// Holds only the cancellation flag of the running search, if any — mirrors
/// `ScanState`'s minimal shape. Starting a new search replaces (and so
/// cancels) any earlier one, the same as `start_scan`.
pub struct DuplicatesState {
    current: Mutex<Option<Arc<AtomicBool>>>,
    cleanup: Mutex<Option<Arc<AtomicBool>>>,
}

pub(crate) struct CleanupOperation<'a> {
    active: &'a Mutex<Option<Arc<AtomicBool>>>,
    lifecycle: crate::operations::Operation,
    _guard: crate::operations::OperationGuard,
}
impl CleanupOperation<'_> {
    pub(crate) fn finish(&self, incomplete: bool) {
        self.lifecycle.finish(incomplete);
    }
}
impl Drop for CleanupOperation<'_> {
    fn drop(&mut self) {
        *lock(self.active) = None;
    }
}

impl DuplicatesState {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
            cleanup: Mutex::new(None),
        }
    }

    pub(crate) fn begin_cleanup(&self) -> AppResult<(CleanupOperation<'_>, Arc<AtomicBool>)> {
        let mut active = lock(&self.cleanup);
        if active.is_some() {
            return Err(AppError::new("duplicateCleanupBusy"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let lifecycle =
            crate::operations::Operation::start("duplicateCleanup", None, cancel.clone())?;
        let guard = lifecycle.guard();
        *active = Some(Arc::clone(&cancel));
        Ok((
            CleanupOperation {
                active: &self.cleanup,
                lifecycle,
                _guard: guard,
            },
            cancel,
        ))
    }
}

impl Default for DuplicatesState {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts a duplicate search over `roots`. Each must canonicalise to a local
/// drive path — the same check `start_scan` applies to its own root, with no
/// dependency on a prior scan having run. Runs on a background thread;
/// results and progress arrive through the `duplicates:*` events, matching
/// `scan:*`.
#[tauri::command(async)]
#[expect(
    clippy::too_many_arguments,
    reason = "Explicit Tauri command state and validated scan options"
)]
pub fn find_duplicates(
    app: AppHandle,
    state: State<DuplicatesState>,
    result_roots: State<guard::ResultRoots>,
    scan_id: u64,
    roots: Vec<String>,
    excludes: Vec<String>,
    use_cache: bool,
    options: Option<DuplicateFilter>,
) -> AppResult<()> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let lifecycle =
        crate::operations::Operation::start("duplicates", Some(scan_id), cancelled.clone())?;
    crate::operation_ipc::attach(&lifecycle, &app);
    let operation_guard = lifecycle.guard();
    lifecycle
        .start_readonly_watchdog(Duration::from_secs(20))
        .map_err(AppError::from)?;
    if let Some(previous) = lock(&state.current).replace(cancelled.clone()) {
        crate::operations::cancellation_requested(&previous);
    }
    let filter = options.unwrap_or_default().normalized()?;
    let mut canonical_roots = Vec::with_capacity(roots.len());
    for root in &roots {
        lifecycle.touch();
        let canonical = fs::canonicalize(root)
            .map_err(|error| AppError::with_detail("pathNotReadable", error))?;
        if !is_local_drive_path(&canonical) {
            return Err(AppError::new("localDrivesOnly"));
        }
        canonical_roots.push(canonical);
    }

    canonical_roots.sort();
    canonical_roots.dedup();
    result_roots.register(
        guard::ResultKind::Duplicates,
        scan_id,
        canonical_roots.clone(),
    );
    let excludes_lower: Vec<String> = excludes
        .iter()
        .map(|exclude| exclude.trim().to_lowercase())
        .filter(|exclude| !exclude.is_empty())
        .collect();

    let worker_lifecycle = lifecycle.clone();
    thread::Builder::new()
        .name("duplicates-search".into())
        .spawn(move || {
            run_search(
                app,
                scan_id,
                canonical_roots,
                SearchPolicy {
                    excludes_lower,
                    use_cache,
                    filter,
                },
                cancelled,
                worker_lifecycle,
            )
        })
        .map_err(AppError::from)?;
    operation_guard.handoff();
    Ok(())
}

/// Cancels the running search, if any.
#[tauri::command]
pub fn cancel_duplicates(state: State<DuplicatesState>) {
    if let Some(cancelled) = lock(&state.current).as_ref() {
        crate::operations::cancellation_requested(cancelled);
    }
}

#[tauri::command]
pub fn cancel_duplicate_cleanup(state: State<DuplicatesState>) {
    if let Some(cancel) = lock(&state.cleanup).as_ref() {
        crate::operations::cancellation_requested(cancel);
    }
}

/// The background job: walks every root into one shared size-bucket map (so
/// a file that exists identically on two different drives is correctly
/// reported as a cross-drive duplicate too), then hashes and confirms
/// candidates. Emits `duplicates:progress` throughout and always finishes
/// with a `duplicates:done` — including when cancelled partway through,
/// carrying whatever groups were already confirmed by that point rather than
/// discarding them: a cancelled walk (nothing hashed yet) naturally ends up
/// with an empty list, but a cancelled analyze phase still reports every
/// group that had already been hashed and byte-confirmed before the
/// cancellation took effect.
struct SearchPolicy {
    excludes_lower: Vec<String>,
    use_cache: bool,
    filter: DuplicateFilter,
}

fn run_search(
    app: AppHandle,
    scan_id: u64,
    roots: Vec<PathBuf>,
    policy: SearchPolicy,
    cancelled: Arc<AtomicBool>,
    lifecycle: crate::operations::Operation,
) {
    let SearchPolicy {
        excludes_lower,
        use_cache,
        filter,
    } = policy;
    let _operation_guard = lifecycle.guard();
    let diagnostics = Diagnostics::default();
    let cache_access = use_cache.then(cache::scan_access);
    if use_cache {
        let _ = cache::prepare_scan();
    } // Migrate once before worker connections open.
    let mut by_size: HashMap<u64, Vec<Candidate>> = HashMap::new();
    let mut files_scanned = 0u64;
    let mut bytes_scanned = 0u64;
    let mut last_emit = Instant::now() - PROGRESS_INTERVAL;

    for root in &roots {
        collect_files_diagnosed(
            root,
            &excludes_lower,
            1,
            &mut by_size,
            &cancelled,
            &mut |path, size| {
                files_scanned += 1;
                bytes_scanned += size;
                if last_emit.elapsed() >= PROGRESS_INTERVAL {
                    crate::operations::touch_token(&cancelled);
                    emit_progress(
                        &app,
                        scan_id,
                        "walk",
                        files_scanned,
                        bytes_scanned,
                        path,
                        0,
                        0,
                        &diagnostics,
                    );
                    last_emit = Instant::now();
                }
            },
            &filter,
            &diagnostics,
        );
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
    }

    let candidates_total: u64 = by_size
        .values()
        .filter(|files| files.len() >= 2)
        .map(|files| files.len() as u64)
        .sum();
    let candidates_hashed = AtomicU64::new(0);
    let current_analyze_path = Mutex::new(String::new());

    // A ticker thread reports analyze-phase progress from the shared counters
    // below while the hash workers run, the same split responsibility
    // `scan.rs`'s worker pool + ticker thread already uses for the primary
    // scan. `hash_and_confirm` itself never touches Tauri or events; it only
    // updates `candidates_hashed`/`current_analyze_path`.
    let confirmed_groups = thread::scope(|scope| {
        let (finished, completion) = std::sync::mpsc::channel::<()>();
        let app = &app;
        let cancelled = &cancelled;
        let candidates_hashed = &candidates_hashed;
        let current_analyze_path = &current_analyze_path;
        let diagnostics = &diagnostics;
        let ticker = scope.spawn(move || {
            loop {
                match completion.recv_timeout(PROGRESS_INTERVAL) {
                    Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }
                let done = candidates_hashed.load(Ordering::Relaxed);
                let path = lock(current_analyze_path).clone();
                emit_progress(
                    app,
                    scan_id,
                    "analyze",
                    files_scanned,
                    bytes_scanned,
                    Path::new(&path),
                    candidates_total,
                    done,
                    diagnostics,
                );
            }
        });

        let result = hash_and_confirm_diagnosed(
            by_size,
            cancelled,
            candidates_hashed,
            current_analyze_path,
            use_cache,
            &|confirmed| {
                let group = duplicate_group(confirmed);
                let _ = app.emit(
                    EVENT_GROUP,
                    GroupPayload {
                        scan_id,
                        group: &group,
                    },
                );
            },
            diagnostics,
        );
        let _ = finished.send(());
        let _ = ticker.join();
        result
    });

    let mut groups = Vec::new();
    for confirmed in confirmed_groups {
        if confirmed.len() < 2 {
            continue;
        }
        groups.push(duplicate_group(&confirmed));
    }

    groups.sort_unstable_by_key(|group| {
        std::cmp::Reverse(group.size * (group.files.len() as u64 - 1))
    });
    let was_cancelled = cancelled.load(Ordering::Relaxed);
    drop(cache_access);
    if use_cache {
        cache::finish_scan();
    }
    lifecycle.finish(was_cancelled || diagnostics.snapshot().coverage == "partial");
    let _ = app.emit(
        EVENT_DONE,
        DonePayload {
            scan_id,
            groups: &groups,
            cancelled: was_cancelled,
            diagnostics: diagnostics.snapshot(),
        },
    );
}

/// CPU ceiling; analysis_workers applies the stricter HDD/USB/SSD/NVMe limit.
/// Warm file caches must not be mistaken for a benefit of extra reader threads.
fn analyze_worker_count() -> usize {
    let cores = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (cores * 2).clamp(4, 24)
}

/// Converts a confirmed group to the IPC model. Paths keep their verbatim form for filesystem safety.
fn duplicate_group(confirmed: &[Candidate]) -> DuplicateGroup {
    DuplicateGroup {
        size: confirmed[0].size,
        sample_hash: confirmed[0]
            .sample_hash
            .map(|hash| format!("{hash:016x}"))
            .unwrap_or_default(),
        files: confirmed
            .iter()
            .map(|candidate| {
                let cleanup_reason = crate::delete::duplicate_cleanup_problem(&candidate.path);
                DuplicateFile {
                    path: candidate.path.to_string_lossy().into_owned(),
                    name: candidate.name.clone(),
                    modified: candidate.modified,
                    cleanup_eligible: cleanup_eligible(&candidate.path) && cleanup_reason.is_none(),
                    cleanup_reason,
                    allocated_bytes: candidate.storage.map(|value| value.allocated),
                    reclaimable_bytes: candidate
                        .storage
                        .map(|value| if value.links == 1 { value.allocated } else { 0 }),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
fn hash_and_confirm(
    by_size: HashMap<u64, Vec<Candidate>>,
    cancelled: &AtomicBool,
    done: &AtomicU64,
    current: &Mutex<String>,
    use_cache: bool,
) -> Vec<Vec<Candidate>> {
    hash_and_confirm_streaming(by_size, cancelled, done, current, use_cache, &|_| {})
}

/// Independent size/hash buckets finish and publish without waiting for unrelated buckets.
/// Nested Rayon hashing keeps even one huge size bucket parallel; connections belong to workers.
#[cfg(test)]
fn hash_and_confirm_streaming(
    by_size: HashMap<u64, Vec<Candidate>>,
    cancelled: &AtomicBool,
    done: &AtomicU64,
    current_path: &Mutex<String>,
    use_cache: bool,
    on_group: &(impl Fn(&[Candidate]) + Sync),
) -> Vec<Vec<Candidate>> {
    hash_and_confirm_diagnosed(
        by_size,
        cancelled,
        done,
        current_path,
        use_cache,
        on_group,
        &Diagnostics::default(),
    )
}

fn hash_and_confirm_diagnosed(
    by_size: HashMap<u64, Vec<Candidate>>,
    cancelled: &AtomicBool,
    done: &AtomicU64,
    current_path: &Mutex<String>,
    use_cache: bool,
    on_group: &(impl Fn(&[Candidate]) + Sync),
    diagnostics: &Diagnostics,
) -> Vec<Vec<Candidate>> {
    if cancelled.load(Ordering::Relaxed) || by_size.values().all(|files| files.len() < 2) {
        return Vec::new();
    }
    thread_local! { static CACHE: RefCell<Option<(Option<rusqlite::Connection>, Vec<cache::CacheEntry>)>> = const { RefCell::new(None) }; }
    let workers = analysis_workers(&by_size);
    let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(workers).build() else {
        // Thread exhaustion must not turn a real search into an empty successful result.
        let mut confirmed = Vec::new();
        for files in by_size.into_values().filter(|files| files.len() >= 2) {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let mut buckets: HashMap<u64, Vec<Candidate>> = HashMap::new();
            for mut candidate in files {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                *lock(current_path) = candidate.path.to_string_lossy().into_owned();
                if sample_candidate(&mut candidate, None, cancelled).is_ok() {
                    buckets
                        .entry(candidate.sample_hash.unwrap())
                        .or_default()
                        .push(candidate);
                } else {
                    done.fetch_add(1, Ordering::Relaxed);
                }
            }
            for candidates in buckets.into_values() {
                let groups = split_by_exact_content(
                    candidates,
                    cancelled,
                    current_path,
                    on_group,
                    done,
                    diagnostics,
                );
                for group in groups.into_iter().filter(|group| group.len() >= 2) {
                    on_group(&group);
                    confirmed.push(group);
                }
            }
        }
        return confirmed;
    };
    let confirmed = pool.install(|| {
        by_size
            .into_par_iter()
            .filter(|(_, files)| files.len() >= 2)
            .flat_map(|(_, files)| {
                let hashed: Vec<_> = files
                    .into_par_iter()
                    .filter_map(|mut candidate| {
                        if cancelled.load(Ordering::Relaxed) {
                            return None;
                        }
                        let path = candidate.path.to_string_lossy();
                        *lock(current_path) = path.to_string();
                        let hash = CACHE.with(|cell| {
                            let mut slot = cell.borrow_mut();
                            if use_cache && slot.is_none() {
                                *slot = Some((cache::open().ok(), Vec::new()));
                            }
                            if cancelled.load(Ordering::Relaxed) {
                                return None;
                            }
                            let conn = if use_cache {
                                slot.as_ref().and_then(|(conn, _)| conn.as_ref())
                            } else {
                                None
                            };
                            let entry = match sample_candidate(&mut candidate, conn, cancelled) {
                                Ok(entry) => entry,
                                Err(error) => {
                                    diagnostics.error(&error);
                                    return None;
                                }
                            };
                            if let Some((Some(conn), pending)) = slot.as_mut() {
                                if let Some(entry) = entry {
                                    pending.push(entry);
                                }
                                if pending.len() >= 128 {
                                    let _ = cache::store_batch(conn, pending);
                                    pending.clear();
                                }
                            }
                            candidate.sample_hash
                        });
                        candidate.sample_hash = hash;
                        Some((hash, candidate))
                    })
                    .collect();
                let mut buckets: HashMap<u64, Vec<Candidate>> = HashMap::new();
                for (hash, candidate) in hashed {
                    if let Some(hash) = hash {
                        buckets.entry(hash).or_default().push(candidate);
                    } else {
                        done.fetch_add(1, Ordering::Relaxed);
                    }
                }
                buckets
                    .into_par_iter()
                    .flat_map(|(_, candidates)| {
                        let groups = split_by_exact_content(
                            candidates,
                            cancelled,
                            current_path,
                            on_group,
                            done,
                            diagnostics,
                        );
                        for group in &groups {
                            if group.len() >= 2 {
                                on_group(group);
                            }
                        }
                        groups
                            .into_iter()
                            .filter(|group| group.len() >= 2)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    });
    // Close every thread-local connection explicitly before announcing completion.
    // Relying on asynchronous worker shutdown can leave a large WAL after a scan.
    pool.broadcast(|_| {
        CACHE.with(|cell| {
            if let Some((Some(conn), pending)) = cell.borrow_mut().take()
                && !cancelled.load(Ordering::Relaxed)
            {
                let _ = cache::store_batch(&conn, &pending);
            }
        })
    });
    confirmed
}

/// Random readers are constrained by the slowest selected medium, not CPU count.
fn analysis_workers(by_size: &HashMap<u64, Vec<Candidate>>) -> usize {
    let letters: HashSet<char> = by_size
        .values()
        .flat_map(|files| files.iter())
        .filter_map(|candidate| match candidate.path.components().next() {
            Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    Some(letter as char)
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    letters
        .into_iter()
        .map(|letter| {
            let (media, bus) = crate::drives::media_and_bus_type(letter);
            workers_for_medium(media, bus)
        })
        .min()
        .unwrap_or(2)
        .min(analyze_worker_count())
}

fn workers_for_medium(media: &str, bus: &str) -> usize {
    if bus.eq_ignore_ascii_case("nvme") {
        12
    } else if bus.eq_ignore_ascii_case("usb") {
        if media.eq_ignore_ascii_case("HDD") {
            1
        } else {
            2
        }
    } else if media.eq_ignore_ascii_case("SSD") {
        6
    } else if media.eq_ignore_ascii_case("HDD") {
        1
    } else {
        2
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Bounded progress snapshot fields are emitted together"
)]
fn emit_progress(
    app: &AppHandle,
    scan_id: u64,
    phase: &'static str,
    files_scanned: u64,
    bytes_scanned: u64,
    current_path: &Path,
    candidates_total: u64,
    candidates_done: u64,
    diagnostics: &Diagnostics,
) {
    let current_path = current_path.to_string_lossy();
    let _ = app.emit(
        EVENT_PROGRESS,
        ProgressPayload {
            scan_id,
            phase,
            files_scanned,
            bytes_scanned,
            current_path: &current_path,
            candidates_total,
            candidates_done,
            diagnostics: diagnostics.snapshot(),
        },
    );
}

/// Walks `dir` and buckets every file it finds by size directly into
/// `by_size`, so a size with only one file never has to be hashed at all.
/// `on_file` is called once per discovered file with its size, before it's
/// known whether it will turn out to be a duplicate — the caller uses it
/// purely for progress reporting (a running byte total is what lets the walk
/// phase show a real, size-based percentage instead of a guess — see
/// `run_search`). An unreadable subdirectory is skipped rather than failing
/// the whole search, same as the main scanner treats access-denied folders.
#[cfg(test)]
fn collect_files_filtered(
    dir: &Path,
    excludes_lower: &[String],
    depth: u32,
    by_size: &mut HashMap<u64, Vec<Candidate>>,
    cancelled: &AtomicBool,
    on_file: &mut dyn FnMut(&Path, u64),
    filter: &DuplicateFilter,
) {
    collect_files_diagnosed(
        dir,
        excludes_lower,
        depth,
        by_size,
        cancelled,
        on_file,
        filter,
        &Diagnostics::default(),
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "Recursive enumeration shares cancellation, diagnostics and candidate budget"
)]
fn collect_files_diagnosed(
    dir: &Path,
    excludes_lower: &[String],
    depth: u32,
    by_size: &mut HashMap<u64, Vec<Candidate>>,
    cancelled: &AtomicBool,
    on_file: &mut dyn FnMut(&Path, u64),
    filter: &DuplicateFilter,
    diagnostics: &Diagnostics,
) {
    if cancelled.load(Ordering::Relaxed) {
        return;
    }
    if depth > MAX_DEPTH {
        diagnostics.depth_limited.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let Ok(entries) = crate::scan_directory::read_dir_standard(dir) else {
        diagnostics.unreadable.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let exclusions = crate::scan::directory_exclusions(dir, excludes_lower);
    if exclusions.is_excluded("") {
        return;
    }
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                diagnostics.error(&error);
                continue;
            }
        };
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        let Ok(file_type) = entry.file_type() else {
            diagnostics.unreadable.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let (size, cache_stamp, denied) = entry.file_details_precise();
        if file_type.is_file() {
            if denied || size == 0 {
                if denied {
                    diagnostics.unreadable.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
            if !filter.accepts_size(size) && !exclusions.has_leaf_exclusions() {
                diagnostics.excluded.fetch_add(1, Ordering::Relaxed);
                on_file(dir, size);
                continue;
            }
        }
        let raw_name = entry.file_name();
        if exclusions.has_leaf_exclusions() && exclusions.is_excluded(&raw_name.to_string_lossy()) {
            continue;
        }
        if file_type.is_dir() {
            collect_files_diagnosed(
                &dir.join(&raw_name),
                excludes_lower,
                depth + 1,
                by_size,
                cancelled,
                on_file,
                filter,
                diagnostics,
            );
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if denied || size == 0 {
            continue;
        }
        if !filter.accepts(Path::new(raw_name.as_os_str()), size) {
            diagnostics.excluded.fetch_add(1, Ordering::Relaxed);
            // Count discovered files without allocating a full path for every excluded file.
            on_file(dir, size);
            continue;
        }
        let budget = 192 + (dir.as_os_str().len() + raw_name.len()) as u64 * 4;
        if !diagnostics.reserve_candidate(budget, MAX_CANDIDATES, MAX_CANDIDATE_BYTES) {
            on_file(dir, size);
            continue;
        }
        let path = dir.join(&raw_name);
        on_file(&path, size);
        by_size.entry(size).or_default().push(Candidate {
            path,
            name: raw_name.to_string_lossy().into_owned(),
            size,
            modified: cache_stamp / 1_000_000_000,
            cache_stamp,
            sample_hash: None,
            file_id: None,
            storage: None,
        });
    }
}

/// Recheck content and precise metadata immediately before a duplicate deletion.
#[cfg(test)]
pub(crate) fn verify_cleanup_pair(
    target: &Path,
    keeper: &Path,
    expected_size: u64,
) -> AppResult<()> {
    if target
        .to_string_lossy()
        .eq_ignore_ascii_case(&keeper.to_string_lossy())
    {
        return Err(AppError::new("duplicateChanged"));
    }
    let target_before = fs::metadata(target)?;
    let keeper_before = fs::metadata(keeper)?;
    if !target_before.is_file()
        || !keeper_before.is_file()
        || target_before.len() != expected_size
        || keeper_before.len() != expected_size
    {
        return Err(AppError::new("duplicateChanged"));
    }
    crate::delete::ensure_plain_single_link_file(&File::open(target)?)?;
    crate::delete::ensure_plain_single_link_file(&File::open(keeper)?)?;
    if !files_equal(target, keeper, &AtomicBool::new(false))? {
        return Err(AppError::new("duplicateChanged"));
    }
    let target_after = fs::metadata(target)?;
    let keeper_after = fs::metadata(keeper)?;
    if target_before.len() != target_after.len()
        || keeper_before.len() != keeper_after.len()
        || target_before.modified()? != target_after.modified()?
        || keeper_before.modified()? != keeper_after.modified()?
    {
        return Err(AppError::new("duplicateChanged"));
    }
    Ok(())
}

/// Test entry point for the production start/middle/end prefilter.
#[cfg(test)]
fn hash_file(path: &Path, size: u64) -> std::io::Result<u64> {
    let mut file = File::open(path)?;
    sample_open_file(&mut file, size, &AtomicBool::new(false))
}

fn interrupted(cancelled: &AtomicBool) -> std::io::Result<()> {
    crate::operations::touch_token(cancelled);
    if cancelled.load(Ordering::Relaxed) {
        return Err(std::io::ErrorKind::Interrupted.into());
    }
    Ok(())
}

fn modified_stamp(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_nanos()).ok())
        .unwrap_or(0)
}

/// Read at most 24 KiB. Start/middle/end separates common headers and long shared prefixes.
/// Sampling is only a prefilter; final matches still require every byte to agree.
fn sample_open_file(file: &mut File, size: u64, cancelled: &AtomicBool) -> std::io::Result<u64> {
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; HASH_CHUNK_BYTES];
    let mut positions = vec![0];
    if size > HASH_CHUNK_BYTES as u64 {
        positions.push((size - HASH_CHUNK_BYTES as u64) / 2);
        positions.push(size - HASH_CHUNK_BYTES as u64);
    }
    for offset in positions {
        interrupted(cancelled)?;
        file.seek(SeekFrom::Start(offset))?;
        let length = (size - offset).min(HASH_CHUNK_BYTES as u64) as usize;
        file.read_exact(&mut buffer[..length])?;
        offset.hash(&mut hasher);
        buffer[..length].hash(&mut hasher);
    }
    interrupted(cancelled)?;
    Ok(hasher.finish())
}

fn sample_candidate(
    candidate: &mut Candidate,
    conn: Option<&rusqlite::Connection>,
    cancelled: &AtomicBool,
) -> std::io::Result<Option<cache::CacheEntry>> {
    interrupted(cancelled)?;
    let mut file = File::open(&candidate.path)?;
    let before = file.metadata()?;
    if !before.is_file()
        || before.len() != candidate.size
        || modified_stamp(&before) != candidate.cache_stamp
    {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
        };
        let mut info: FILE_ID_INFO = unsafe { std::mem::zeroed() };
        // SAFETY: the owned file handle is live and info is a writable matching structure.
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle().cast(),
                FileIdInfo,
                (&mut info as *mut FILE_ID_INFO).cast(),
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            )
        } != 0
        {
            // ReFS needs the full 128-bit identity; its truncated 64-bit index is not unique.
            if info.FileId.Identifier != [0; 16] {
                candidate.file_id = Some((info.VolumeSerialNumber, info.FileId.Identifier));
            }
        }
    }
    let path = candidate.path.to_string_lossy();
    candidate.storage = crate::storage::from_file(&file, &candidate.path).ok();
    // A preserved timestamp is not proof of the same file after replacement.
    // Cache only identities returned by the opened object; unsupported IDs miss safely.
    let cache_key = candidate.file_id.map(|(volume, identity)| {
        let id = identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("{path}\0{volume:016x}:{id}")
    });
    let cached = if candidate.cache_stamp == 0 || cache_key.is_none() {
        None
    } else {
        conn.and_then(|connection| {
            cache::lookup(
                connection,
                cache_key.as_deref().unwrap(),
                candidate.size,
                candidate.cache_stamp,
            )
        })
    };
    let hash = match cached {
        Some(hash) => hash,
        None => sample_open_file(&mut file, candidate.size, cancelled)?,
    };
    let after = file.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    interrupted(cancelled)?;
    candidate.sample_hash = Some(hash);
    Ok(
        (conn.is_some() && cached.is_none() && candidate.cache_stamp != 0 && cache_key.is_some())
            .then(|| cache::CacheEntry {
                path: cache_key.unwrap(),
                size: candidate.size,
                modified: candidate.cache_stamp,
                hash,
            }),
    )
}

/// Splits a same-size, same-hash bucket into groups that are actually
/// byte-for-byte identical, comparing each candidate only against the first
/// member of each group so far — sound because file equality is transitive.
fn split_by_exact_content(
    candidates: Vec<Candidate>,
    cancelled: &AtomicBool,
    current_path: &Mutex<String>,
    on_group: &(impl Fn(&[Candidate]) + Sync),
    done: &AtomicU64,
    diagnostics: &Diagnostics,
) -> Vec<Vec<Candidate>> {
    struct Checked<'a>(&'a AtomicU64);
    impl Drop for Checked<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    let cache_representative = candidates.len() >= 4
        && candidates
            .first()
            .is_some_and(|file| file.size <= MAX_REPRESENTATIVE_BYTES && file.cache_stamp != 0);
    // One bounded, in-memory representative per bucket avoids rereading it for every copy.
    // This never persists data and cleanup still performs a fresh comparison from disk.
    let mut representative: Option<Vec<u8>> = None;
    let mut identities = HashSet::new();
    let mut fingerprints: Option<HashMap<u64, Vec<usize>>> = None;
    let mut last_publish = Instant::now();
    'candidates: for candidate in candidates {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let _checked = Checked(done);
        // Hard-link names are one physical file, not additional reclaimable copies.
        if let Some(id) = candidate.file_id
            && !identities.insert(id)
        {
            continue;
        }
        *lock(current_path) = candidate.path.to_string_lossy().into_owned();
        // Most buckets contain real copies: keep the cheap linear representative comparison.
        // After four different contents, full fingerprints bound the collision workload.
        if fingerprints.is_none() && groups.len() >= 4 {
            let mut index: HashMap<u64, Vec<usize>> = HashMap::new();
            for (i, group) in groups.iter().enumerate() {
                if let Ok(hash) = full_fingerprint(&group[0], cancelled) {
                    index.entry(hash).or_default().push(i);
                }
            }
            fingerprints = Some(index);
        }
        let hash = if fingerprints.is_some() {
            match full_fingerprint(&candidate, cancelled) {
                Ok(hash) => Some(hash),
                Err(_) => continue,
            }
        } else {
            None
        };
        let possible: Vec<usize> = match hash {
            Some(hash) => fingerprints
                .as_ref()
                .unwrap()
                .get(&hash)
                .cloned()
                .unwrap_or_default(),
            None => (0..groups.len()).collect(),
        };
        for index in possible {
            let group = &mut groups[index];
            if index == 0 && cache_representative && representative.is_none() {
                representative = read_representative(&group[0], cancelled).ok();
            }
            let same = if index == 0 {
                representative.as_ref().map(|bytes| {
                    matches_representative(&candidate, &group[0], bytes, cancelled).unwrap_or_else(
                        |error| {
                            diagnostics.error(&error);
                            false
                        },
                    )
                })
            } else {
                None
            };
            if same.unwrap_or_else(|| {
                files_equal_checked(
                    &candidate.path,
                    &group[0].path,
                    cancelled,
                    Some((candidate.size, candidate.cache_stamp, group[0].cache_stamp)),
                )
                .unwrap_or_else(|error| {
                    diagnostics.error(&error);
                    false
                })
            }) {
                group.push(candidate);
                // Grow the live result too; previously copy counts stayed at two until completion.
                if group.len() == 2
                    || group.len().is_power_of_two()
                    || last_publish.elapsed() >= PROGRESS_INTERVAL
                {
                    on_group(group);
                    last_publish = Instant::now();
                }
                continue 'candidates;
            }
        }
        if let Some(hash) = hash {
            fingerprints
                .as_mut()
                .unwrap()
                .entry(hash)
                .or_default()
                .push(groups.len());
        }
        groups.push(vec![candidate]);
    }
    groups
}

fn read_representative(candidate: &Candidate, cancelled: &AtomicBool) -> std::io::Result<Vec<u8>> {
    interrupted(cancelled)?;
    let mut file = File::open(&candidate.path)?;
    let before = file.metadata()?;
    if before.len() != candidate.size || modified_stamp(&before) != candidate.cache_stamp {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0; candidate.size as usize];
    for chunk in bytes.chunks_mut(COMPARE_CHUNK_BYTES) {
        interrupted(cancelled)?;
        file.read_exact(chunk)?;
    }
    let after = file.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

fn matches_representative(
    candidate: &Candidate,
    keeper: &Candidate,
    bytes: &[u8],
    cancelled: &AtomicBool,
) -> std::io::Result<bool> {
    interrupted(cancelled)?;
    let original = File::open(&keeper.path)?;
    let mut file = File::open(&candidate.path)?;
    let keeper_before = original.metadata()?;
    let before = file.metadata()?;
    if before.len() != bytes.len() as u64
        || keeper_before.len() != bytes.len() as u64
        || modified_stamp(&before) != candidate.cache_stamp
        || modified_stamp(&keeper_before) != keeper.cache_stamp
    {
        return Ok(false);
    }
    let mut buffer = vec![0; COMPARE_CHUNK_BYTES];
    for chunk in bytes.chunks(COMPARE_CHUNK_BYTES) {
        interrupted(cancelled)?;
        file.read_exact(&mut buffer[..chunk.len()])?;
        if &buffer[..chunk.len()] != chunk {
            return Ok(false);
        }
    }
    let after = file.metadata()?;
    let keeper_after = original.metadata()?;
    Ok(before.len() == after.len()
        && keeper_before.len() == keeper_after.len()
        && before.modified()? == after.modified()?
        && keeper_before.modified()? == keeper_after.modified()?)
}

fn full_fingerprint(candidate: &Candidate, cancelled: &AtomicBool) -> std::io::Result<u64> {
    interrupted(cancelled)?;
    let mut file = File::open(&candidate.path)?;
    let before = file.metadata()?;
    if before.len() != candidate.size || modified_stamp(&before) != candidate.cache_stamp {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut buffer = vec![0u8; COMPARE_CHUNK_BYTES];
    // This is a collision prefilter, not a security digest. Byte equality remains authoritative.
    let mut hasher = DefaultHasher::new();
    let mut remaining = candidate.size;
    while remaining > 0 {
        interrupted(cancelled)?;
        let length = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..length])?;
        hasher.write(&buffer[..length]);
        remaining -= length as u64;
    }
    let after = file.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    interrupted(cancelled)?;
    Ok(hasher.finish())
}

#[cfg(test)]
fn files_equal(a: &Path, b: &Path, cancelled: &AtomicBool) -> std::io::Result<bool> {
    files_equal_checked(a, b, cancelled, None)
}

fn files_equal_checked(
    a: &Path,
    b: &Path,
    cancelled: &AtomicBool,
    expected: Option<(u64, u64, u64)>,
) -> std::io::Result<bool> {
    interrupted(cancelled)?;
    let mut file_a = File::open(a)?;
    let mut file_b = File::open(b)?;
    let before_a = file_a.metadata()?;
    let before_b = file_b.metadata()?;
    if !before_a.is_file() || !before_b.is_file() || before_a.len() != before_b.len() {
        return Ok(false);
    }
    if let Some((size, stamp_a, stamp_b)) = expected
        && (before_a.len() != size
            || modified_stamp(&before_a) != stamp_a
            || modified_stamp(&before_b) != stamp_b)
    {
        return Ok(false);
    }
    let mut buffer_a = [0u8; COMPARE_CHUNK_BYTES];
    let mut buffer_b = [0u8; COMPARE_CHUNK_BYTES];
    let mut remaining = before_a.len();
    while remaining > 0 {
        if cancelled.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Duplicate search cancelled",
            ));
        }
        let length = remaining.min(COMPARE_CHUNK_BYTES as u64) as usize;
        file_a.read_exact(&mut buffer_a[..length])?;
        interrupted(cancelled)?;
        file_b.read_exact(&mut buffer_b[..length])?;
        if buffer_a[..length] != buffer_b[..length] {
            return Ok(false);
        }
        remaining -= length as u64;
    }
    let after_a = file_a.metadata()?;
    let after_b = file_b.metadata()?;
    Ok(before_a.len() == after_a.len()
        && before_b.len() == after_b.len()
        && before_a.modified()? == after_a.modified()?
        && before_b.modified()? == after_b.modified()?)
}

/* ── Event payloads ───────────────────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload<'a> {
    scan_id: u64,
    /// `"walk"` while discovering files, `"analyze"` while hashing/comparing
    /// candidates. `candidatesTotal`/`candidatesDone` are `0` during `"walk"`.
    phase: &'static str,
    files_scanned: u64,
    /// Total bytes of every file discovered so far. Lets the frontend show a
    /// real percentage for the walk phase (`bytesScanned` / a drive's own
    /// used size, the same metric `start_scan`'s progress bar already uses)
    /// instead of a size-based duration guess — which turned out to be a bad
    /// model here, since neither the walk (metadata only) nor the analyze
    /// phase (now a bounded sample per file, not the whole file) actually
    /// reads through a drive's used space the way the guess assumed.
    bytes_scanned: u64,
    current_path: &'a str,
    candidates_total: u64,
    candidates_done: u64,
    diagnostics: DiagnosticSnapshot,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GroupPayload<'a> {
    scan_id: u64,
    group: &'a DuplicateGroup,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DonePayload<'a> {
    scan_id: u64,
    groups: &'a [DuplicateGroup],
    /// Whether this search was cancelled before it finished on its own —
    /// `groups` may be an incomplete picture of the selected drives when this
    /// is `true`, which the frontend uses to word the results differently.
    cancelled: bool,
    diagnostics: DiagnosticSnapshot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering as AtomicOrdering;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn cleanup_operation_is_exclusive_and_releases_its_cancel_token() {
        let state = DuplicatesState::new();
        let (operation, cancel) = state.begin_cleanup().unwrap();
        assert!(state.begin_cleanup().is_err());
        lock(&state.cleanup)
            .as_ref()
            .unwrap()
            .store(true, Ordering::Relaxed);
        assert!(cancel.load(Ordering::Relaxed));
        drop(operation);
        let (_, next) = state.begin_cleanup().unwrap();
        assert!(!next.load(Ordering::Relaxed));
    }

    #[test]
    fn unreadable_and_too_deep_searches_are_explicitly_partial() {
        let fixture = TempDir::new();
        let diagnostics = Diagnostics::default();
        let mut buckets = HashMap::new();
        let cancelled = AtomicBool::new(false);
        collect_files_diagnosed(
            &fixture.0.join("missing"),
            &[],
            1,
            &mut buckets,
            &cancelled,
            &mut |_, _| {},
            &DuplicateFilter::default(),
            &diagnostics,
        );
        collect_files_diagnosed(
            &fixture.0,
            &[],
            MAX_DEPTH + 1,
            &mut buckets,
            &cancelled,
            &mut |_, _| {},
            &DuplicateFilter::default(),
            &diagnostics,
        );
        assert_eq!(diagnostics.snapshot().unreadable, 1);
        assert_eq!(diagnostics.snapshot().depth_limited, 1);
        assert_eq!(diagnostics.snapshot().coverage, "partial");
    }

    #[test]
    fn stale_candidates_are_counted_as_changed_in_the_final_diagnostics() {
        let fixture = TempDir::new();
        fixture.write("a", b"same");
        fixture.write("b", b"same");
        let mut buckets = HashMap::new();
        collect_files(
            &fixture.0,
            &[],
            1,
            &mut buckets,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        for candidate in buckets.values_mut().flatten() {
            candidate.cache_stamp = 1;
        }
        let diagnostics = Diagnostics::default();
        let groups = hash_and_confirm_diagnosed(
            buckets,
            &AtomicBool::new(false),
            &AtomicU64::new(0),
            &Mutex::new(String::new()),
            false,
            &|_| {},
            &diagnostics,
        );
        assert!(groups.is_empty());
        assert_eq!(diagnostics.snapshot().changed, 2);
        assert_eq!(diagnostics.snapshot().coverage, "partial");
    }

    #[test]
    fn reader_limits_follow_the_medium() {
        assert_eq!(workers_for_medium("SSD", "NVMe"), 12);
        assert_eq!(workers_for_medium("SSD", "SATA"), 6);
        assert_eq!(workers_for_medium("HDD", "SATA"), 1);
        assert_eq!(workers_for_medium("HDD", "USB"), 1);
        assert_eq!(workers_for_medium("SSD", "USB"), 2);
        assert_eq!(workers_for_medium("Unknown", "Unknown"), 2);
    }

    #[test]
    fn samples_catch_middle_and_tail_changes_and_obey_cancellation() {
        let dir = TempDir::new();
        let original = vec![1u8; 1024 * 1024];
        let a = dir.write("a", &original);
        for offset in [512 * 1024, original.len() - 1] {
            let mut changed = original.clone();
            changed[offset] = 2;
            let b = dir.write("b", &changed);
            assert_ne!(
                hash_file(&a, original.len() as u64).unwrap(),
                hash_file(&b, original.len() as u64).unwrap()
            );
        }
        assert_eq!(
            sample_open_file(
                &mut File::open(a).unwrap(),
                original.len() as u64,
                &AtomicBool::new(true)
            )
            .unwrap_err()
            .kind(),
            std::io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn adaptive_fingerprints_preserve_copies_when_samples_all_collide() {
        let dir = TempDir::new();
        for group in 0..12 {
            let mut bytes = vec![0u8; 512 * 1024];
            bytes[128 * 1024] = group;
            for copy in 0..3 {
                dir.write(&format!("g{group}-{copy}.bin"), &bytes);
            }
        }
        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 12);
        assert!(groups.iter().all(|group| group.files.len() == 3));
    }

    #[test]
    fn hard_link_names_do_not_inflate_duplicate_count() {
        let dir = TempDir::new();
        let a = dir.write("a", b"same");
        dir.write("b", b"same");
        fs::hard_link(a, dir.0.join("alias")).unwrap();
        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }

    #[test]
    fn stale_candidate_metadata_cannot_produce_a_confirmed_group() {
        let dir = TempDir::new();
        dir.write("a", b"same");
        dir.write("b", b"same");
        let mut buckets = HashMap::new();
        collect_files(
            &dir.0,
            &[],
            1,
            &mut buckets,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        for files in buckets.values_mut() {
            for file in files {
                file.cache_stamp = 1;
            }
        }
        let groups = hash_and_confirm(
            buckets,
            &AtomicBool::new(false),
            &AtomicU64::new(0),
            &Mutex::new(String::new()),
            false,
        );
        assert!(groups.is_empty());
    }

    #[test]
    fn comparison_rejects_a_changed_size_instead_of_matching_two_shrunk_files() {
        let dir = TempDir::new();
        let a = dir.write("a", b"same");
        let b = dir.write("b", b"same");
        assert!(!files_equal_checked(&a, &b, &AtomicBool::new(false), Some((8, 0, 0))).unwrap());
    }

    #[test]
    fn in_memory_representative_checks_changes_and_cancellation() {
        let dir = TempDir::new();
        dir.write("a", b"same");
        dir.write("b", b"same");
        let mut buckets = HashMap::new();
        collect_files(
            &dir.0,
            &[],
            1,
            &mut buckets,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        let files = buckets.remove(&4).unwrap();
        let bytes = read_representative(&files[0], &AtomicBool::new(false)).unwrap();
        assert!(
            matches_representative(&files[1], &files[0], &bytes, &AtomicBool::new(false)).unwrap()
        );
        assert!(
            matches_representative(&files[1], &files[0], &bytes, &AtomicBool::new(true)).is_err()
        );
        fs::write(&files[0].path, b"changed").unwrap();
        assert!(
            !matches_representative(&files[1], &files[0], &bytes, &AtomicBool::new(false)).unwrap()
        );
    }

    #[test]
    fn cached_samples_are_invalidated_after_an_edit_within_the_same_second() {
        let dir = TempDir::new();
        let path = dir.write("a", b"same");
        let file = File::options().write(true).open(&path).unwrap();
        let timestamp = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        file.set_times(
            std::fs::FileTimes::new().set_modified(timestamp + Duration::from_nanos(100)),
        )
        .unwrap();
        let first = modified_stamp(&file.metadata().unwrap());
        file.set_times(
            std::fs::FileTimes::new().set_modified(timestamp + Duration::from_nanos(200)),
        )
        .unwrap();
        let second = modified_stamp(&file.metadata().unwrap());
        assert_eq!(first / 1_000_000_000, second / 1_000_000_000);
        assert_ne!(first, second);
    }

    #[test]
    fn replacement_with_same_length_and_timestamp_does_not_reuse_cached_sample() {
        let dir = TempDir::new();
        let path = dir.write("a", b"same");
        let timestamp = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let stamp = |p: &Path| {
            File::options()
                .write(true)
                .open(p)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(timestamp))
                .unwrap();
        };
        stamp(&path);
        let candidate = |p: &Path| {
            let metadata = fs::metadata(p).unwrap();
            Candidate {
                path: p.to_path_buf(),
                name: "a".into(),
                size: metadata.len(),
                modified: 1_700_000_000,
                cache_stamp: modified_stamp(&metadata),
                sample_hash: None,
                file_id: None,
                storage: None,
            }
        };
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE file_hashes(path TEXT PRIMARY KEY,size INTEGER,modified INTEGER,hash INTEGER,sample_version INTEGER,last_seen INTEGER)").unwrap();
        let mut first = candidate(&path);
        let entry = sample_candidate(&mut first, Some(&conn), &AtomicBool::new(false))
            .unwrap()
            .unwrap();
        cache::store_batch(&conn, &[entry]).unwrap();
        fs::rename(&path, dir.0.join("retained-old-object")).unwrap();
        fs::write(&path, b"diff").unwrap();
        stamp(&path);
        let mut replacement = candidate(&path);
        assert_eq!(first.size, replacement.size);
        assert_eq!(first.cache_stamp, replacement.cache_stamp);
        let new_entry =
            sample_candidate(&mut replacement, Some(&conn), &AtomicBool::new(false)).unwrap();
        assert!(
            new_entry.is_some(),
            "replacement must miss the old identity cache"
        );
        assert_ne!(first.file_id, replacement.file_id);
        assert_ne!(first.sample_hash, replacement.sample_hash);
    }

    /// A scratch directory under the system temp folder, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "discreveal-duplicates-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, name: &str, contents: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, contents).unwrap();
            path
        }

        fn subdir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Walks with no size/type filter and no excludes beyond what's passed — the test-only
    /// equivalent of `collect_files_filtered` with `DuplicateFilter::default()`.
    fn collect_files(
        dir: &Path,
        excludes_lower: &[String],
        depth: u32,
        by_size: &mut HashMap<u64, Vec<Candidate>>,
        cancelled: &AtomicBool,
        on_file: &mut dyn FnMut(&Path, u64),
    ) {
        collect_files_filtered(
            dir,
            excludes_lower,
            depth,
            by_size,
            cancelled,
            on_file,
            &DuplicateFilter::default(),
        );
    }

    #[test]
    fn scan_filters_apply_size_and_included_or_excluded_extensions_before_hashing() {
        let dir = TempDir::new();
        dir.write("a.PDF", b"1234");
        dir.write("b.pdf", b"1234");
        dir.write("c.jpg", b"12345");
        dir.write("d.pdf", b"123456");
        let run = |mode| {
            let filter = DuplicateFilter {
                min_bytes: 4,
                max_bytes: 5,
                type_mode: mode,
                extensions: vec!["pdf".into()],
            };
            let mut buckets = HashMap::new();
            collect_files_filtered(
                &dir.0,
                &[],
                1,
                &mut buckets,
                &AtomicBool::new(false),
                &mut |_, _| {},
                &filter,
            );
            buckets
                .into_values()
                .flatten()
                .map(|file| file.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(run(TypeMode::Include).len(), 2);
        assert_eq!(run(TypeMode::Exclude), vec!["c.jpg"]);
        assert_eq!(run(TypeMode::All).len(), 3);
    }

    #[test]
    fn cleanup_requires_an_unchanged_distinct_retained_original() {
        let dir = TempDir::new();
        let keeper = dir.write("original.txt", b"same");
        let copy = dir.write("copy.txt", b"same");
        assert!(verify_cleanup_pair(&copy, &keeper, 4).is_ok());
        assert!(verify_cleanup_pair(&keeper, &keeper, 4).is_err());
        fs::write(&copy, b"edit").unwrap();
        assert!(verify_cleanup_pair(&copy, &keeper, 4).is_err());
        fs::write(&copy, b"same").unwrap();
        fs::remove_file(&keeper).unwrap();
        assert!(verify_cleanup_pair(&copy, &keeper, 4).is_err());
        assert!(copy.exists());
    }

    #[test]
    fn cleanup_rejects_hard_links_and_bulk_suggestions_exclude_application_data() {
        let dir = TempDir::new();
        let keeper = dir.write("original.txt", b"same");
        let copy = dir.write("copy.txt", b"same");
        fs::hard_link(&copy, dir.0.join("hardlink.txt")).unwrap();
        assert!(verify_cleanup_pair(&copy, &keeper, 4).is_err());
        assert!(cleanup_eligible(Path::new(
            r"C:\Users\Someone\Documents\report.pdf"
        )));
        assert!(!cleanup_eligible(Path::new(
            r"C:\Users\Someone\AppData\cache\report.pdf"
        )));
        assert!(!cleanup_eligible(Path::new(
            r"C:\source\node_modules\readme.txt"
        )));
        assert!(!cleanup_eligible(Path::new(
            r"C:\Users\Someone\Documents\app.exe"
        )));
    }

    /// Runs the search synchronously (no ticker, no events) for tests, using
    /// the same parallel `hash_and_confirm` the live command uses.
    fn find(dirs: &[&Path], excludes: &[&str]) -> Vec<DuplicateGroup> {
        let excludes_lower: Vec<String> = excludes.iter().map(|e| e.to_lowercase()).collect();
        let cancelled = AtomicBool::new(false);
        let mut by_size = HashMap::new();
        for dir in dirs {
            collect_files(
                dir,
                &excludes_lower,
                1,
                &mut by_size,
                &cancelled,
                &mut |_, _| {},
            );
        }
        let candidates_hashed = AtomicU64::new(0);
        let current_path = Mutex::new(String::new());
        let mut groups = Vec::new();
        for confirmed in hash_and_confirm(
            by_size,
            &cancelled,
            &candidates_hashed,
            &current_path,
            false,
        ) {
            if confirmed.len() >= 2 {
                groups.push(duplicate_group(&confirmed));
            }
        }
        groups
    }

    #[test]
    fn streamed_match_survives_cancellation_before_the_bucket_finishes() {
        let dir = TempDir::new();
        for name in ["a", "b", "c", "d"] {
            dir.write(name, b"same");
        }
        let cancelled = AtomicBool::new(false);
        let mut buckets = HashMap::new();
        collect_files(&dir.0, &[], 1, &mut buckets, &cancelled, &mut |_, _| {});
        let events = Mutex::new(Vec::new());
        let groups = hash_and_confirm_streaming(
            buckets,
            &cancelled,
            &AtomicU64::new(0),
            &Mutex::new(String::new()),
            false,
            &|group| {
                lock(&events).push(group.len());
                cancelled.store(true, Ordering::Relaxed);
            },
        );
        assert!(
            !lock(&events).is_empty(),
            "a group must arrive before search completion"
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].len(),
            2,
            "cancel must retain the published pair and stop remaining comparisons"
        );
    }

    #[test]
    fn byte_comparison_obeys_cancellation() {
        let dir = TempDir::new();
        dir.write("a", b"same");
        dir.write("b", b"same");
        let error =
            files_equal(&dir.0.join("a"), &dir.0.join("b"), &AtomicBool::new(true)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    }

    #[test]
    fn identical_files_are_grouped() {
        let dir = TempDir::new();
        dir.write("a.txt", b"hello world");
        dir.write("b.txt", b"hello world");
        dir.write("c.txt", b"something else entirely");

        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].size, 11);
        assert_eq!(groups[0].files.len(), 2);
    }

    #[test]
    fn same_size_different_content_is_not_a_duplicate() {
        let dir = TempDir::new();
        dir.write("a.bin", b"AAAAAAAAAA");
        dir.write("b.bin", b"BBBBBBBBBB");

        assert!(find(&[&dir.0], &[]).is_empty());
    }

    #[test]
    fn zero_byte_files_are_never_reported() {
        let dir = TempDir::new();
        dir.write("empty1.txt", b"");
        dir.write("empty2.txt", b"");

        assert!(find(&[&dir.0], &[]).is_empty());
    }

    #[test]
    fn a_lone_file_is_not_a_group_of_one() {
        let dir = TempDir::new();
        dir.write("alone.txt", b"unique content");

        assert!(find(&[&dir.0], &[]).is_empty());
    }

    #[test]
    fn excluded_folders_are_skipped() {
        let dir = TempDir::new();
        dir.write("keep.txt", b"duplicate me");
        let excluded = dir.subdir("node_modules");
        fs::write(excluded.join("copy.txt"), b"duplicate me").unwrap();

        assert!(find(&[&dir.0], &["node_modules"]).is_empty());
    }

    #[test]
    fn duplicates_across_subfolders_are_found() {
        let dir = TempDir::new();
        let sub1 = dir.subdir("one");
        let sub2 = dir.subdir("two");
        fs::write(sub1.join("photo.jpg"), b"same bytes here").unwrap();
        fs::write(sub2.join("photo-copy.jpg"), b"same bytes here").unwrap();

        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }

    #[test]
    fn three_way_duplicate_forms_one_group_not_three_pairs() {
        let dir = TempDir::new();
        dir.write("a.txt", b"triplicate");
        dir.write("b.txt", b"triplicate");
        dir.write("c.txt", b"triplicate");

        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 3);
    }

    #[test]
    fn cancellation_stops_the_walk_early() {
        let dir = TempDir::new();
        for i in 0..50 {
            dir.write(&format!("file{i}.txt"), format!("content {i}").as_bytes());
        }
        let cancelled = AtomicBool::new(true); // already cancelled before the walk starts
        let mut by_size = HashMap::new();
        let mut seen = 0;
        collect_files(&dir.0, &[], 1, &mut by_size, &cancelled, &mut |_, _| {
            seen += 1
        });
        assert_eq!(seen, 0);
        assert!(by_size.is_empty());
    }

    #[test]
    fn a_search_cancelled_before_any_hashing_reports_no_groups() {
        // Nothing was ever compared, so an honest result is "found nothing yet" rather than a
        // panic or a leftover partial answer from before cancellation was even requested.
        let dir = TempDir::new();
        dir.write("a.txt", b"same");
        dir.write("b.txt", b"same");

        let cancelled = AtomicBool::new(false);
        let mut by_size = HashMap::new();
        collect_files(&dir.0, &[], 1, &mut by_size, &cancelled, &mut |_, _| {});

        cancelled.store(true, Ordering::Relaxed);
        let candidates_hashed = AtomicU64::new(0);
        let current_path = Mutex::new(String::new());
        let groups = hash_and_confirm(
            by_size,
            &cancelled,
            &candidates_hashed,
            &current_path,
            false,
        );
        assert!(groups.is_empty());
    }

    #[test]
    fn cancelling_after_hashing_finished_still_returns_the_confirmed_groups() {
        // `run_search` only learns a search was cancelled *after* `hash_and_confirm` has already
        // produced its result — this proves that result is still returned, not thrown away, once
        // every candidate has actually been hashed and confirmed byte-for-byte by the time the
        // cancellation flag is checked. Regression test for the bug the user reported: cancelling
        // used to discard groups that had already been found.
        let dir = TempDir::new();
        dir.write("a.txt", b"duplicate content");
        dir.write("b.txt", b"duplicate content");
        dir.write("c.txt", b"unrelated");

        let cancelled = AtomicBool::new(false);
        let mut by_size = HashMap::new();
        collect_files(&dir.0, &[], 1, &mut by_size, &cancelled, &mut |_, _| {});

        let candidates_hashed = AtomicU64::new(0);
        let current_path = Mutex::new(String::new());
        // Cancellation arrives only *after* hashing/confirming has fully run — hash_and_confirm
        // itself takes no cancellation shortcut once it already has a result in hand.
        let groups = hash_and_confirm(
            by_size,
            &cancelled,
            &candidates_hashed,
            &current_path,
            false,
        );
        cancelled.store(true, Ordering::Relaxed);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    #[test]
    fn multiple_roots_are_merged_into_one_search() {
        let dir_a = TempDir::new();
        let dir_b = TempDir::new();
        dir_a.write("one.dat", b"cross-root duplicate");
        dir_b.write("two.dat", b"cross-root duplicate");

        let groups = find(&[&dir_a.0, &dir_b.0], &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }

    #[test]
    fn hashed_candidates_are_counted_once_each() {
        let dir = TempDir::new();
        dir.write("a.txt", b"same");
        dir.write("b.txt", b"same");
        dir.write("c.txt", b"same");
        dir.write("solo.txt", b"unique, not a candidate");

        let excludes_lower: Vec<String> = Vec::new();
        let cancelled = AtomicBool::new(false);
        let mut by_size = HashMap::new();
        collect_files(
            &dir.0,
            &excludes_lower,
            1,
            &mut by_size,
            &cancelled,
            &mut |_, _| {},
        );

        let candidates_total: u64 = by_size
            .values()
            .filter(|files| files.len() >= 2)
            .map(|files| files.len() as u64)
            .sum();
        assert_eq!(candidates_total, 3); // solo.txt's size group has only 1 member, never hashed

        let candidates_hashed = AtomicU64::new(0);
        let current_path = Mutex::new(String::new());
        hash_and_confirm(
            by_size,
            &cancelled,
            &candidates_hashed,
            &current_path,
            false,
        );
        assert_eq!(candidates_hashed.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn files_sharing_a_long_prefix_but_differing_later_are_not_duplicates() {
        // Shared headers must not hide differing tails. The multi-position sample
        // rejects these files before confirmation; unsampled differences are covered
        // separately by adaptive_fingerprints_preserve_copies_when_samples_all_collide.
        let dir = TempDir::new();
        let shared_prefix = vec![0x41u8; HASH_SAMPLE_BYTES + 4096];
        let mut a = shared_prefix.clone();
        let mut b = shared_prefix.clone();
        a.extend_from_slice(b"first file's tail");
        b.extend_from_slice(b"second file's tail!!");
        // Same size is required for the two to even reach the hashing stage together.
        a.resize(a.len().max(b.len()), 0);
        b.resize(a.len(), 0);
        dir.write("a.bin", &a);
        dir.write("b.bin", &b);

        assert!(find(&[&dir.0], &[]).is_empty());
    }

    #[test]
    fn parallel_hashing_stays_correct_across_many_worker_chunks() {
        // Enough candidates to exercise parallel sampling and regrouping across
        // many size buckets, instead of trivially fitting in one worker task.
        let dir = TempDir::new();
        for group in 0..20 {
            let content = format!("group {group} shared content").repeat(50);
            for copy in 0..5 {
                dir.write(&format!("g{group}_copy{copy}.dat"), content.as_bytes());
            }
        }
        for i in 0..100 {
            dir.write(
                &format!("unique{i}.dat"),
                format!("nothing else like unique file {i}").as_bytes(),
            );
        }

        let groups = find(&[&dir.0], &[]);
        assert_eq!(groups.len(), 20);
        for group in &groups {
            assert_eq!(group.files.len(), 5);
        }
    }
}
