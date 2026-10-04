//! Parallel directory scanner.
//!
//! A scan splits the root into its top-level directories and distributes
//! them over a pool of worker threads through a shared pull queue. Each
//! worker walks one subtree at a time and reports it as soon as it is
//! complete, so the frontend can render partial results while the scan runs.
//!
//! Cancellation is cooperative: workers check a flag between filesystem
//! calls. A thread blocked inside a kernel call (for example on a file locked
//! by another process) cannot be interrupted, so a watchdog reports a stalled
//! scan and abandons it instead of killing the thread.
//!
//! Every event carries the id of the scan it belongs to, so the frontend can
//! discard events of a scan that has been cancelled or replaced.

use crate::diagnostics::{Diagnostics, Snapshot as DiagnosticSnapshot};
use crate::error::{AppError, AppResult};
use crate::guard;
use crate::scan_directory;
pub(crate) use crate::scan_policy::{DirectoryFilter, directory_exclusions};
#[cfg(test)]
use crate::scan_policy::{HDD_WORKERS, NVME_WORKERS, SSD_WORKERS, USB_WORKERS};
use crate::scan_policy::{ScanFilter, limit_to_bytes, worker_count_for_drive};
#[allow(unused_imports)] // External benchmark harness imports the reference matcher via scan.
pub(crate) use crate::scan_policy::{is_excluded, mark_markinson_contains_at_boundaries};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

/// Upper bound of children kept per directory; the remainder is folded into
/// one synthetic node.
const MAX_CHILDREN: usize = 100;

/// The largest children of every directory are always kept.
const ALWAYS_KEPT_CHILDREN: usize = 12;

/// Beyond [`ALWAYS_KEPT_CHILDREN`] a child is only kept if it holds at least
/// `1 / SMALL_CHILD_DIVISOR` of its parent's size. Directories consist mostly
/// of many tiny entries; folding them keeps the result small enough to
/// transfer and render for a full system drive without changing any total.
const SMALL_CHILD_DIVISOR: u64 = 100;

/// Maximum directory depth. Symlinks and junctions are never followed, so
/// this only guards against pathologically deep trees.
pub(crate) const MAX_DEPTH: u32 = 60;

/// Time without any filesystem progress after which a scan is reported as
/// stalled and abandoned.
const STALL_TIMEOUT: Duration = Duration::from_secs(20);

/// Interval of the ticker thread, which drives the stall check and the
/// progress and partial-tree events.
const TICK_INTERVAL: Duration = Duration::from_millis(500);

/// Minimum time between partial-tree events.
const PARTIAL_MIN_INTERVAL: Duration = Duration::from_millis(500);

/// Time between two partial events as a multiple of the time the earlier one
/// took to build and send.
const PARTIAL_COST_FACTOR: u32 = 3;

const EVENT_PROGRESS: &str = "scan:progress";
const EVENT_PARTIAL: &str = "scan:partial";
const EVENT_DONE: &str = "scan:done";
const EVENT_ERROR: &str = "scan:error";

/* ── Tree model ───────────────────────────────────── */

/// One file or directory in the scan result. Children are reference-counted
/// so partial snapshots of the root can share completed subtrees.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScanNode {
    name: String,
    path: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    logical_size: Option<u64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    allocation_unknown: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    shared_storage: bool,
    is_dir: bool,
    children: Vec<Arc<ScanNode>>,
    /// Last-modified time as Unix seconds; `0` for directories and for files
    /// whose metadata could not be read. Used only by the age chart.
    modified: u64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    access_denied: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    limit_reached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_offset: Option<usize>,
    /// Directory that is still being scanned; `size` is what was found so far.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    in_progress: bool,
    /// For the synthetic overflow node: how many entries it stands for. The
    /// frontend builds the display name in the user's language.
    #[serde(skip_serializing_if = "Option::is_none")]
    overflow_count: Option<usize>,
}

impl ScanNode {
    fn file(name: String, path: String, size: u64, modified: u64, access_denied: bool) -> Self {
        Self {
            name,
            path,
            size,
            logical_size: None,
            allocation_unknown: false,
            shared_storage: false,
            is_dir: false,
            children: Vec::new(),
            modified,
            access_denied,
            limit_reached: false,
            next_offset: None,
            in_progress: false,
            overflow_count: None,
        }
    }

    fn dir(
        name: &str,
        path: &Path,
        size: u64,
        children: Vec<Arc<ScanNode>>,
        access_denied: bool,
    ) -> Self {
        let logical_size = children
            .iter()
            .any(|node| node.logical_size.is_some())
            .then(|| {
                children
                    .iter()
                    .map(|node| node.logical_size.unwrap_or(node.size))
                    .sum()
            });
        let allocation_unknown = children.iter().any(|node| node.allocation_unknown);
        Self {
            name: name.to_owned(),
            path: path.to_string_lossy().into_owned(),
            size,
            logical_size,
            allocation_unknown,
            shared_storage: false,
            is_dir: true,
            children,
            modified: 0,
            access_denied,
            limit_reached: false,
            next_offset: None,
            in_progress: false,
            overflow_count: None,
        }
    }

    /// Placeholder for a directory whose scan has not finished yet.
    fn in_progress(name: &str, path: &Path, size_so_far: u64) -> Self {
        Self {
            in_progress: true,
            ..Self::dir(name, path, size_so_far, Vec::new(), false)
        }
    }

    /// Synthetic node summarising folded children. It has no path and no name.
    fn overflow(count: usize, size: u64) -> Self {
        Self {
            overflow_count: Some(count),
            ..Self::file(String::new(), String::new(), size, 0, false)
        }
    }
}

/// Sorts children by size (descending) and folds insignificant ones into a
/// single overflow node. `parent_size` is the size of the directory itself.
///
/// A full sort is `O(n log n)`; most directories have far fewer entries than
/// `MAX_CHILDREN`, where that cost is trivial either way, but a handful of
/// real directories (`WinSxS`, a bloated `node_modules`, a cache folder) hold
/// many thousands of entries — and no matter how many, at most
/// `MAX_CHILDREN` of them can ever survive folding. For anything larger than
/// that, `select_nth_unstable_by_key` finds exactly that front slice in
/// average `O(n)` without placing the (irrelevant, about-to-be-folded) rest
/// in any particular order, and only the front slice itself then needs an
/// actual sort — `O(MAX_CHILDREN log MAX_CHILDREN)`, a constant regardless
/// of how large the directory is.
fn sort_and_cap(children: &mut Vec<Arc<ScanNode>>, parent_size: u64) {
    if children.len() > MAX_CHILDREN {
        children.select_nth_unstable_by_key(MAX_CHILDREN - 1, |node| {
            Reverse((node.size, node.name.len()))
        });
    }
    let sorted_len = children.len().min(MAX_CHILDREN);
    children[..sorted_len].sort_unstable_by_key(|node| Reverse((node.size, node.name.len())));

    let significant_size = parent_size / SMALL_CHILD_DIVISOR;
    let kept = children
        .iter()
        .enumerate()
        .take_while(|(index, node)| {
            *index < ALWAYS_KEPT_CHILDREN
                || (*index < MAX_CHILDREN && node.size > 0 && node.size >= significant_size)
        })
        .count();

    // Folding a single entry would hide it without saving anything.
    if children.len() - kept > 1 {
        let tail = children.split_off(kept);
        let tail_size = tail.iter().map(|node| node.size).sum();
        let mut overflow = ScanNode::overflow(tail.len(), tail_size);
        overflow.logical_size = tail
            .iter()
            .any(|node| node.logical_size.is_some())
            .then(|| {
                tail.iter()
                    .map(|node| node.logical_size.unwrap_or(node.size))
                    .sum()
            });
        overflow.allocation_unknown = tail.iter().any(|node| node.allocation_unknown);
        children.push(Arc::new(overflow));
    }
}

/* ── Scan run state ───────────────────────────────── */

struct ScanRun {
    filter: ScanFilter,
    storage: Option<Arc<crate::storage::Ledger>>,
    started: Instant,
    scanned_files: AtomicU64,
    scanned_bytes: AtomicU64,
    /// Milliseconds since `started` at the last observed filesystem activity.
    last_activity_ms: AtomicU64,
    current_dir: Mutex<String>,
    cancelled: Arc<AtomicBool>,
    diagnostics: Diagnostics,
    lifecycle: Option<crate::operations::Operation>,
}

impl ScanRun {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        crate::operations::cancellation_requested(&self.cancelled);
    }

    fn touch(&self) {
        self.last_activity_ms
            .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    fn idle_for(&self) -> Duration {
        self.started.elapsed().saturating_sub(Duration::from_millis(
            self.last_activity_ms.load(Ordering::Relaxed),
        ))
    }

    fn enter_dir(&self, dir: &Path) {
        self.touch();
        *lock(&self.current_dir) = dir.to_string_lossy().into_owned();
    }
}

/// Directory-local progress avoids contending on four shared cache lines for
/// every file. Publish small batches, or after 100 ms even in a slow listing.
/// Drop also publishes the final partial batch, including on cancellation.
struct FileProgress<'a> {
    run: &'a ScanRun,
    subtree_bytes: &'a AtomicU64,
    files: u64,
    bytes: u64,
    published_at: Instant,
}

impl<'a> FileProgress<'a> {
    fn new(run: &'a ScanRun, subtree_bytes: &'a AtomicU64) -> Self {
        Self {
            run,
            subtree_bytes,
            files: 0,
            bytes: 0,
            published_at: Instant::now(),
        }
    }

    fn record(&mut self, size: u64) {
        self.files += 1;
        self.bytes += size;
        if self.files >= 64 || self.published_at.elapsed() >= Duration::from_millis(100) {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.files == 0 {
            return;
        }
        self.run
            .scanned_files
            .fetch_add(self.files, Ordering::Relaxed);
        self.run
            .scanned_bytes
            .fetch_add(self.bytes, Ordering::Relaxed);
        self.subtree_bytes.fetch_add(self.bytes, Ordering::Relaxed);
        self.run.touch();
        self.files = 0;
        self.bytes = 0;
        self.published_at = Instant::now();
    }
}

impl Drop for FileProgress<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

/// A top-level directory of the scan root. Workers scan it as one unit and
/// publish the bytes found so far, so the frontend can show it growing.
struct TopLevelDir {
    path: PathBuf,
    name: String,
    bytes_so_far: AtomicU64,
    result: Mutex<Option<Arc<ScanNode>>>,
}

/// Everything belonging to one scan: shared counters, the work queue and the
/// results collected so far.
struct ScanJob {
    id: u64,
    run: ScanRun,
    /// Canonical scan root, used to validate later paths from the frontend.
    root: PathBuf,
    root_name: String,
    root_path: String,
    root_files: Vec<Arc<ScanNode>>,
    top_levels: Vec<TopLevelDir>,
    /// Size of the scan-scoped rayon thread pool (see `spawn_scan_pool`).
    worker_count: usize,
    done_emitted: AtomicBool,
    /// Earliest time of the next partial-tree event.
    next_partial_at: Mutex<Instant>,
    /// Completed top-level subtrees successfully sent in an earlier snapshot.
    sent_subtrees: Mutex<HashSet<String>>,
}

impl ScanJob {
    /// Builds the root node from its own files, every finished top-level
    /// directory and a live placeholder for each unfinished one. The root is
    /// never pruned by the size filter; it only folds if it holds an
    /// unusually large number of entries.
    fn assemble_root(&self) -> ScanNode {
        let mut children = self.root_files.clone();
        for top in &self.top_levels {
            let finished = lock(&top.result).clone();
            children.push(finished.unwrap_or_else(|| {
                let logical = top.bytes_so_far.load(Ordering::Relaxed);
                let mut node = ScanNode::in_progress(&top.name, &top.path, logical);
                if self.run.storage.is_some() {
                    node.logical_size = Some(logical);
                    node.size = 0;
                    node.allocation_unknown = true;
                }
                Arc::new(node)
            }));
        }
        let size = children.iter().map(|node| node.size).sum();
        if children.len() > MAX_CHILDREN {
            sort_and_cap(&mut children, size);
        } else {
            children.sort_unstable_by_key(|node| Reverse(node.size));
        }
        let mut root = ScanNode::dir(
            &self.root_name,
            Path::new(&self.root_path),
            size,
            children,
            false,
        );
        if self.run.storage.is_some() {
            root.logical_size.get_or_insert(0);
            root.allocation_unknown |= self.run.diagnostics.snapshot().coverage == "partial";
        }
        root
    }

    fn emit_progress(&self, app: &AppHandle) {
        let payload = ProgressPayload {
            scan_id: self.id,
            scanned_files: self.run.scanned_files.load(Ordering::Relaxed),
            scanned_bytes: self.run.scanned_bytes.load(Ordering::Relaxed),
            current_path: lock(&self.run.current_dir).clone(),
            diagnostics: self.run.diagnostics.snapshot(),
        };
        let _ = app.emit(EVENT_PROGRESS, payload);
    }

    /// Completed subtrees are immutable during a scan. Send them once, then
    /// refer to their paths; the frontend reuses those exact node objects.
    /// The done event always carries the complete independent result tree.
    fn assemble_partial(&self) -> (ScanNode, Vec<String>, Vec<String>) {
        let mut tree = self.assemble_root();
        let sent = lock(&self.sent_subtrees);
        let mut reused = Vec::new();
        let mut fresh = Vec::new();
        for child in &mut tree.children {
            if !child.is_dir || child.in_progress {
                continue;
            }
            if sent.contains(&child.path) {
                reused.push(child.path.clone());
                let mut reused_node = ScanNode::dir(
                    &child.name,
                    Path::new(&child.path),
                    child.size,
                    Vec::new(),
                    child.access_denied,
                );
                reused_node.logical_size = child.logical_size;
                reused_node.allocation_unknown = child.allocation_unknown;
                *child = Arc::new(reused_node);
            } else {
                fresh.push(child.path.clone());
            }
        }
        (tree, reused, fresh)
    }

    /// Emits the current tree unless the previous partial is too recent.
    ///
    /// Serialising a large tree takes time, so the next event is scheduled at
    /// a multiple of the time this one took: small snapshots update twice a
    /// second; expensive first deliveries leave more time for scan workers.
    fn emit_partial_adaptive(&self, app: &AppHandle) {
        if Instant::now() < *lock(&self.next_partial_at) {
            return;
        }
        let started = Instant::now();
        let (tree, reused_paths, fresh_paths) = self.assemble_partial();
        if self.run.is_cancelled() {
            return;
        }
        if app
            .emit(
                EVENT_PARTIAL,
                PartialPayload {
                    scan_id: self.id,
                    tree: &tree,
                    reused_paths,
                },
            )
            .is_ok()
        {
            lock(&self.sent_subtrees).extend(fresh_paths);
        }
        *lock(&self.next_partial_at) =
            Instant::now() + PARTIAL_MIN_INTERVAL.max(started.elapsed() * PARTIAL_COST_FACTOR);
    }

    /// Called once, after every top-level directory (and everything nested
    /// under it) has been walked — see `spawn_scan_pool`, whose blocking
    /// `par_iter().for_each()` call only returns once all of that work is
    /// actually done, so there is no multi-worker completion count to track
    /// here any more. `done_emitted` remains as a safety guard rather than
    /// because anything today can call this twice.
    fn scan_finished(&self, app: &AppHandle) {
        if let Some(lifecycle) = &self.run.lifecycle {
            lifecycle.finish(self.run.diagnostics.snapshot().coverage == "partial");
        }
        if self.run.is_cancelled() {
            return;
        }
        if self.done_emitted.swap(true, Ordering::SeqCst) {
            return;
        }
        let tree = self.assemble_root();
        self.run.touch();
        if self.run.is_cancelled() {
            return;
        }
        let _ = app.emit(
            EVENT_DONE,
            DonePayload {
                scan_id: self.id,
                tree: &tree,
                scanned_files: self.run.scanned_files.load(Ordering::Relaxed),
                scanned_bytes: self.run.scanned_bytes.load(Ordering::Relaxed),
                diagnostics: self.run.diagnostics.snapshot(),
            },
        );
    }
}

/* ── Event payloads ───────────────────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    scan_id: u64,
    scanned_files: u64,
    scanned_bytes: u64,
    diagnostics: DiagnosticSnapshot,
    current_path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PartialPayload<'a> {
    scan_id: u64,
    tree: &'a ScanNode,
    /// Direct children whose already-delivered subtrees should be reused.
    reused_paths: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DonePayload<'a> {
    scan_id: u64,
    tree: &'a ScanNode,
    scanned_files: u64,
    scanned_bytes: u64,
    diagnostics: DiagnosticSnapshot,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload {
    scan_id: u64,
    code: &'static str,
    /// Path the scan was working on when it stalled.
    detail: String,
    seconds: u64,
}

/// Locks a mutex, ignoring poisoning: the guarded data stays consistent
/// because no critical section here can leave it half-updated.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/* ── Directory walking ────────────────────────────── */

enum Entry {
    Dir { path: PathBuf, name: String },
    File(ScanNode),
}

/// Classifies a directory entry. Excluded entries, symlinks, junctions and
/// special files yield `None`; files are recorded in the run counters and in
/// `subtree_bytes`, the running total of the top-level directory being scanned.
fn classify(
    entry: &scan_directory::DirectoryEntry,
    filter: &DirectoryFilter<'_>,
    progress: &mut FileProgress<'_>,
) -> Option<Entry> {
    if filter.exclude_all {
        progress
            .run
            .diagnostics
            .excluded
            .fetch_add(1, Ordering::Relaxed);
        return None;
    }
    let name = entry.file_name().to_string_lossy().into_owned();
    if filter.is_excluded(&name) {
        progress
            .run
            .diagnostics
            .excluded
            .fetch_add(1, Ordering::Relaxed);
        return None;
    }
    let file_type = match entry.file_type() {
        Ok(kind) => kind,
        Err(error) => {
            progress.run.diagnostics.error(&error);
            return None;
        }
    };
    // On Windows this also covers junctions and other name-surrogate reparse
    // points. Skipping them keeps the scan a tree and avoids double counting.
    if file_type.is_symlink() {
        return None;
    }
    let path = entry.path();

    if file_type.is_dir() {
        return Some(Entry::Dir { path, name });
    }
    if file_type.is_file() {
        // On Windows the metadata comes from the directory enumeration
        // itself and costs no extra syscall.
        let (size, modified, denied) = entry.file_details();
        if denied {
            progress
                .run
                .diagnostics
                .unreadable
                .fetch_add(1, Ordering::Relaxed);
        }
        progress.record(size);
        let mut node = ScanNode::file(
            name,
            path.to_string_lossy().into_owned(),
            size,
            modified,
            denied,
        );
        if let Some(ledger) = &progress.run.storage {
            node.logical_size = Some(size);
            match crate::storage::measure(&path).and_then(|value| {
                if value.logical != size {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
                ledger.charge(value, &path)
            }) {
                Ok((allocated, shared)) => {
                    node.size = allocated;
                    node.shared_storage = shared;
                }
                Err(error) => {
                    node.size = 0;
                    node.allocation_unknown = true;
                    if error.kind() == std::io::ErrorKind::OutOfMemory {
                        progress
                            .run
                            .diagnostics
                            .resource_limited
                            .fetch_add(1, Ordering::Relaxed);
                    } else {
                        progress.run.diagnostics.error(&error);
                    }
                }
            }
        }
        return Some(Entry::File(node));
    }
    None
}

/// Walks one directory subtree and returns it with sizes aggregated
/// bottom-up. `depth` is 1 for a top-level directory of the scan root.
///
/// Subdirectories are recursed into through `rayon`'s work-stealing
/// `par_iter`, not a plain sequential loop. A directory listing says nothing
/// about how large each subdirectory actually turns out to be, so a
/// sequential recursion here would let one disproportionately large
/// subdirectory run single-threaded for however long it takes, while every
/// other thread in the pool — having long since finished its own, smaller
/// share — sits idle. Measured on a real system drive: the `Windows` folder
/// alone held roughly as many files as every other top-level folder
/// combined, which meant the scan was effectively single-threaded for most
/// of its total wall time no matter how many worker threads were
/// configured, regardless of which level the pull-queue operated at.
/// `par_iter` fixes this at *every* depth, not just the top level, because
/// rayon's scheduler lets an idle thread steal work from a still-running
/// recursive call anywhere in the tree, not only from a shared top-level
/// queue. It must be called from inside the scan's own `rayon::ThreadPool`
/// (see `spawn_scan_pool`) to respect the drive-type worker count rather
/// than rayon's own default (CPU count) pool.
fn walk(dir: &Path, name: &str, depth: u32, run: &ScanRun, subtree_bytes: &AtomicU64) -> ScanNode {
    if run.is_cancelled() {
        return ScanNode::dir(name, dir, 0, Vec::new(), false);
    }
    if depth > MAX_DEPTH {
        run.diagnostics
            .depth_limited
            .fetch_add(1, Ordering::Relaxed);
        let mut node = ScanNode::dir(name, dir, 0, Vec::new(), false);
        node.limit_reached = true;
        if run.storage.is_some() {
            node.logical_size = Some(0);
            node.allocation_unknown = true;
        }
        return node;
    }
    run.enter_dir(dir);

    // A directory that cannot be listed is almost always an access-denied
    // case; flag it so the UI can explain why it shows 0 B.
    let Ok(entries) = scan_directory::read_dir(dir) else {
        run.diagnostics.unreadable.fetch_add(1, Ordering::Relaxed);
        let mut node = ScanNode::dir(name, dir, 0, Vec::new(), true);
        if run.storage.is_some() {
            node.logical_size = Some(0);
            node.allocation_unknown = true;
        }
        return node;
    };

    let mut children = Vec::new();
    let mut subdirs = Vec::new();
    let mut total_size = 0u64;
    let filter = run.filter.for_directory(dir);
    let mut progress = FileProgress::new(run, subtree_bytes);
    for entry in entries.ordered(run.storage.is_some()) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                run.diagnostics.error(&error);
                continue;
            }
        };
        if run.is_cancelled() {
            break;
        }
        match classify(&entry, &filter, &mut progress) {
            Some(Entry::Dir { path, name }) => subdirs.push((path, name)),
            Some(Entry::File(node)) => {
                total_size += node.size;
                children.push(Arc::new(node));
            }
            None => {}
        }
    }
    drop(progress);

    let visit = |(path, name): (PathBuf, String)| {
        Arc::new(walk(&path, &name, depth + 1, run, subtree_bytes))
    };
    let subdir_nodes: Vec<Arc<ScanNode>> = if run.storage.is_some() {
        subdirs.into_iter().map(visit).collect()
    } else {
        subdirs.into_par_iter().map(visit).collect()
    };
    total_size += subdir_nodes.iter().map(|node| node.size).sum::<u64>();
    children.extend(subdir_nodes);

    let logical = children
        .iter()
        .map(|node| node.logical_size.unwrap_or(node.size))
        .sum();
    let unknown = children.iter().any(|node| node.allocation_unknown)
        || run.diagnostics.snapshot().coverage == "partial";
    if run.filter.is_out_of_range(total_size) {
        children = Vec::new();
    } else {
        sort_and_cap(&mut children, total_size);
    }
    let mut node = ScanNode::dir(name, dir, total_size, children, false);
    if run.storage.is_some() {
        node.logical_size = Some(logical);
        node.allocation_unknown = unknown;
    }
    node
}

/* ── Scan lifecycle ───────────────────────────────── */

/// Holds the most recent scan so it can be cancelled and so its root can
/// validate later paths.
pub struct ScanState {
    accounting: Mutex<HashMap<u64, Arc<crate::storage::Ledger>>>,
    current: Mutex<Option<Arc<ScanJob>>>,
    pending: Mutex<Option<Arc<AtomicBool>>>,
    expansions: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
}

impl ScanState {
    pub fn new() -> Self {
        Self {
            accounting: Mutex::new(HashMap::new()),
            current: Mutex::new(None),
            pending: Mutex::new(None),
            expansions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn begin_scan(&self) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(previous) = lock(&self.pending).replace(Arc::clone(&cancel)) {
            crate::operations::cancellation_requested(&previous);
        }
        if let Some(previous) = lock(&self.current).as_ref() {
            previous.run.cancel();
        }
        cancel
    }

    fn request_cancel(&self) {
        if let Some(cancel) = lock(&self.pending).as_ref() {
            crate::operations::cancellation_requested(cancel);
        }
        if let Some(job) = lock(&self.current).as_ref() {
            job.run.cancel();
        }
    }

    /// Canonical root of the most recent scan, if any.
    pub fn root(&self) -> Option<PathBuf> {
        lock(&self.current).as_ref().map(|job| job.root.clone())
    }
}

impl Default for ScanState {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `path` starts with a drive letter, which excludes network (UNC)
/// and device paths.
pub(crate) fn is_local_drive_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
    )
}

/// Runs the whole scan on a `rayon` thread pool sized to the drive's own
/// worker count (see `worker_count_for_drive`), on one dedicated OS thread
/// so the `#[tauri::command(async)]` call this was spawned from can return
/// immediately. `top_levels.par_iter()` fans every top-level directory out
/// across the pool, and `walk`'s own nested `par_iter` calls (see its doc
/// comment) reuse that exact same pool for everything beneath them, so the
/// configured thread count is an actual ceiling on this scan's concurrency,
/// not just a starting fan-out that a lone worker can later be stuck behind.
fn spawn_scan_pool(app: AppHandle, job: Arc<ScanJob>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("scan-orchestrator".into())
        .spawn(move || {
            let _operation_guard = job
                .run
                .lifecycle
                .as_ref()
                .map(crate::operations::Operation::guard);
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(job.worker_count)
                .thread_name(|index| format!("scan-worker-{index}"))
                .build();
            match pool {
                Ok(pool) => pool.install(|| {
                    let visit = |top: &TopLevelDir| {
                        if job.run.is_cancelled() {
                            return;
                        }
                        let node = walk(&top.path, &top.name, 1, &job.run, &top.bytes_so_far);
                        if !job.run.is_cancelled() {
                            *lock(&top.result) = Some(Arc::new(node));
                        }
                    };
                    if job.run.storage.is_some() {
                        job.top_levels.iter().for_each(visit);
                    } else {
                        job.top_levels.par_iter().for_each(visit);
                    }
                }),
                // Building a same-process thread pool failing at all means the OS
                // is refusing new threads outright (extreme resource exhaustion)
                // — not something a retry or a smaller count would fix. Still
                // worth a real error instead of leaving the frontend stuck on
                // "Scanning…" forever: `scan_finished` only emits `scan:done`,
                // and only when *not* cancelled, so cancelling first and
                // reporting here directly are both necessary.
                Err(error) => {
                    if let Some(lifecycle) = &job.run.lifecycle {
                        lifecycle.fail();
                    }
                    job.run.cancel();
                    let _ = app.emit(
                        EVENT_ERROR,
                        ErrorPayload {
                            scan_id: job.id,
                            code: "scanSpawnFailed",
                            detail: error.to_string(),
                            seconds: 0,
                        },
                    );
                    return;
                }
            }
            job.scan_finished(&app);
        })?;
    Ok(())
}

/// Drives the live updates and the stall check: every tick it emits progress
/// and, at an adaptive pace, the current tree. A scan without filesystem
/// activity for [`STALL_TIMEOUT`] is reported and marked cancelled so the
/// frontend can recover.
fn spawn_ticker(app: AppHandle, job: Arc<ScanJob>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("scan-ticker".into())
        .spawn(move || {
            loop {
                thread::sleep(TICK_INTERVAL);
                if job.run.is_cancelled() || job.done_emitted.load(Ordering::Relaxed) {
                    return;
                }
                if job.run.idle_for() >= STALL_TIMEOUT {
                    if let Some(lifecycle) = &job.run.lifecycle {
                        lifecycle.abandon();
                    }
                    job.run.cancel();
                    let payload = ErrorPayload {
                        scan_id: job.id,
                        code: "scanStalled",
                        detail: lock(&job.run.current_dir).clone(),
                        seconds: STALL_TIMEOUT.as_secs(),
                    };
                    let _ = app.emit(EVENT_ERROR, payload);
                    return;
                }
                job.emit_progress(&app);
                job.emit_partial_adaptive(&app);
            }
        })?;
    Ok(())
}

/// Parameters of a scan as sent by the frontend.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    #[serde(default)]
    measure_allocated: bool,
    /// Chosen by the frontend and echoed in every event of this scan.
    scan_id: u64,
    root_path: String,
    excludes: Vec<String>,
    #[serde(rename = "minSizeMB")]
    min_size_mb: f64,
    #[serde(rename = "maxSizeGB")]
    max_size_gb: f64,
    is_ssd: bool,
    bus_type: String,
}

/// Starts a scan, replacing (and cancelling) any earlier one. Results are
/// delivered through the `scan:*` events.
#[tauri::command(async)]
pub fn start_scan(
    app: AppHandle,
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    options: ScanOptions,
) -> AppResult<()> {
    let ScanOptions {
        measure_allocated,
        scan_id,
        root_path,
        excludes,
        min_size_mb,
        max_size_gb,
        is_ssd,
        bus_type,
    } = options;

    let cancelled = state.begin_scan();
    let lifecycle = crate::operations::Operation::start("scan", Some(scan_id), cancelled.clone())?;
    crate::operation_ipc::attach(&lifecycle, &app);
    let operation_guard = lifecycle.guard();
    let root = PathBuf::from(&root_path);
    let root_real =
        fs::canonicalize(&root).map_err(|error| AppError::with_detail("pathNotReadable", error))?;
    if !is_local_drive_path(&root_real) {
        return Err(AppError::new("localDrivesOnly"));
    }
    if cancelled.load(Ordering::Relaxed) {
        lifecycle.finish(true);
        return Ok(());
    }
    result_roots.register(guard::ResultKind::Scan, scan_id, vec![root_real.clone()]);
    let entries = scan_directory::read_dir(&root_real)
        .map_err(|error| AppError::with_detail("directoryNotReadable", error))?;

    let run = ScanRun {
        storage: measure_allocated.then(|| Arc::new(crate::storage::Ledger::default())),
        filter: ScanFilter {
            excludes_lower: excludes
                .iter()
                .map(|exclude| exclude.trim().to_lowercase())
                .filter(|exclude| !exclude.is_empty())
                .collect(),
            min_size_bytes: limit_to_bytes(min_size_mb, 1024.0 * 1024.0),
            max_size_bytes: limit_to_bytes(max_size_gb, 1024.0 * 1024.0 * 1024.0),
        },
        started: Instant::now(),
        scanned_files: AtomicU64::new(0),
        scanned_bytes: AtomicU64::new(0),
        last_activity_ms: AtomicU64::new(0),
        current_dir: Mutex::new(root_path.clone()),
        cancelled: Arc::clone(&cancelled),
        diagnostics: Diagnostics::default(),
        lifecycle: Some(lifecycle.clone()),
    };

    if let Some(ledger) = &run.storage {
        let mut retained = lock(&state.accounting);
        retained.insert(scan_id, Arc::clone(ledger));
        if retained.len() > 12
            && let Some(oldest) = retained.keys().copied().min()
        {
            retained.remove(&oldest);
        }
    }

    let root_bytes = AtomicU64::new(0);
    let mut top_levels = Vec::new();
    let mut root_files = Vec::new();
    let filter = run.filter.for_directory(&root);
    let mut progress = FileProgress::new(&run, &root_bytes);
    for entry in entries.ordered(measure_allocated) {
        if run.is_cancelled() {
            lifecycle.finish(true);
            return Ok(());
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                run.diagnostics.error(&error);
                continue;
            }
        };
        match classify(&entry, &filter, &mut progress) {
            Some(Entry::Dir { path, name }) => top_levels.push(TopLevelDir {
                path,
                name,
                bytes_so_far: AtomicU64::new(0),
                result: Mutex::new(None),
            }),
            Some(Entry::File(node)) => root_files.push(Arc::new(node)),
            None => {}
        }
    }
    drop(progress);
    // Nested work stealing also uses the pool: a single top-level folder
    // can contain thousands of independent subdirectories.
    let worker_count = if measure_allocated {
        1
    } else {
        worker_count_for_drive(&bus_type, is_ssd)
    };
    let root_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root_path.clone());

    let job = Arc::new(ScanJob {
        id: scan_id,
        run,
        root: root_real,
        root_name,
        root_path,
        root_files,
        top_levels,
        worker_count,
        done_emitted: AtomicBool::new(false),
        next_partial_at: Mutex::new(Instant::now()),
        sent_subtrees: Mutex::new(HashSet::new()),
    });

    let pending = lock(&state.pending);
    if cancelled.load(Ordering::Relaxed)
        || pending
            .as_ref()
            .is_none_or(|token| !Arc::ptr_eq(token, &cancelled))
    {
        lifecycle.finish(true);
        return Ok(());
    }
    if let Some(previous) = lock(&state.current).replace(Arc::clone(&job)) {
        previous.run.cancel();
    }

    drop(pending);
    // The first snapshot goes out at once: every top-level folder is listed
    // immediately and grows while its scan runs.
    job.emit_progress(&app);
    job.emit_partial_adaptive(&app);

    let spawn_error = |error: std::io::Error| {
        job.run.cancel();
        lifecycle.fail();
        AppError::with_detail("scanSpawnFailed", error)
    };
    spawn_ticker(app.clone(), Arc::clone(&job)).map_err(spawn_error)?;
    spawn_scan_pool(app, Arc::clone(&job)).map_err(spawn_error)?;
    operation_guard.handoff();
    Ok(())
}

/// Requests cancellation of the running scan, if any.
#[tauri::command]
pub fn cancel_scan(state: State<ScanState>) {
    state.request_cancel();
}

struct ExpansionOperation {
    active: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    id: u64,
    token: Arc<AtomicBool>,
}
impl Drop for ExpansionOperation {
    fn drop(&mut self) {
        let mut active = lock(&self.active);
        if active
            .get(&self.id)
            .is_some_and(|token| Arc::ptr_eq(token, &self.token))
        {
            active.remove(&self.id);
        }
    }
}

#[tauri::command]
pub(crate) fn cancel_expand_folder(state: State<ScanState>, operation_id: Option<u64>) {
    for (id, token) in lock(&state.expansions).iter() {
        if operation_id.is_none_or(|requested| requested == *id) {
            crate::operations::cancellation_requested(token);
        }
    }
}

/// Re-reads one directory's immediate children, for the "N more items" row
/// `sort_and_cap` leaves behind when a listing is folded. Folded children are
/// not kept in memory — the point of folding is to bound memory and IPC
/// payload for directories with hundreds of tiny entries — so seeing them
/// again means reading the directory a second time rather than looking
/// anything up. Directory sizes can require re-walking large subtrees, so
/// this command runs off the UI thread. It does not respect the active exclusion or size filter,
/// since the point of asking is to see what those settings are hiding.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExpandOptions {
    offset: Option<usize>,
    measure_allocated: Option<bool>,
}

#[tauri::command(async)]
pub(crate) fn expand_folder(
    app: AppHandle,
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    scope: Option<guard::ResultScope>,
    target_path: String,
    operation_id: Option<u64>,
    options: Option<ExpandOptions>,
) -> AppResult<Vec<Arc<ScanNode>>> {
    let ExpandOptions {
        offset,
        measure_allocated,
    } = options.unwrap_or_default();
    let id = operation_id.unwrap_or(0);
    let cancelled = Arc::new(AtomicBool::new(false));
    let lifecycle = crate::operations::Operation::start("scanExpand", Some(id), cancelled.clone())?;
    crate::operation_ipc::attach(&lifecycle, &app);
    let _lifecycle_guard = lifecycle.guard();
    if let Some(previous) = lock(&state.expansions).insert(id, Arc::clone(&cancelled)) {
        crate::operations::cancellation_requested(&previous);
    }
    let _operation = ExpansionOperation {
        active: state.expansions.clone(),
        id,
        token: cancelled.clone(),
    };
    let dir = guard::resolve_for_result(&state, &result_roots, scope, &target_path, true)?;

    let storage = scope
        .and_then(guard::ResultScope::scan_id)
        .and_then(|scan_id| lock(&state.accounting).get(&scan_id).cloned());
    if measure_allocated == Some(true) && storage.is_none() {
        return Err(AppError::new("allocationResultExpired"));
    }
    let run = ScanRun {
        storage,
        filter: ScanFilter {
            excludes_lower: Vec::new(),
            min_size_bytes: 0,
            max_size_bytes: 0,
        },
        started: Instant::now(),
        scanned_files: AtomicU64::new(0),
        scanned_bytes: AtomicU64::new(0),
        last_activity_ms: AtomicU64::new(0),
        current_dir: Mutex::new(String::new()),
        cancelled: Arc::clone(&cancelled),
        diagnostics: Diagnostics::default(),
        lifecycle: None,
    };
    let subtree_bytes = AtomicU64::new(0);

    let entries = scan_directory::read_dir(&dir)
        .map_err(|error| AppError::with_detail("directoryNotReadable", error))?;
    let mut children: Vec<Arc<ScanNode>> = Vec::new();
    let mut overflow_count = 0;
    let mut overflow_size = 0;
    let mut overflow_logical = 0;
    let mut overflow_unknown = false;
    let mut next_offset = None;
    let mut seen = 0usize;
    let offset = offset.unwrap_or(0);
    let letter = match dir.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter as char,
            _ => return Err(AppError::new("localDrivesOnly")),
        },
        _ => return Err(AppError::new("localDrivesOnly")),
    };
    let (media, bus) = crate::drives::media_and_bus_type(letter);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count_for_drive(bus, media == "SSD"))
        .build()
        .map_err(|error| AppError::with_detail("scanSpawnFailed", error))?;
    let filter = run.filter.for_directory(&dir);
    let mut progress = FileProgress::new(&run, &subtree_bytes);
    for entry in entries.ordered(run.storage.is_some()) {
        if run.is_cancelled() {
            lifecycle.finish(true);
            return Err(AppError::new("scanCancelled"));
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                run.diagnostics.error(&error);
                continue;
            }
        };
        if seen < offset {
            seen += 1;
            continue;
        }
        seen += 1;
        let node = match classify(&entry, &filter, &mut progress) {
            Some(Entry::Dir { path, name }) if children.len() < 1000 => {
                pool.install(|| walk(&path, &name, 1, &run, &subtree_bytes))
            }
            Some(Entry::Dir { .. }) => {
                next_offset.get_or_insert(seen - 1);
                overflow_count += 1;
                overflow_unknown |= run.storage.is_some();
                continue;
            }
            Some(Entry::File(node)) => node,
            None => continue,
        };
        if children.len() < 1000 {
            children.push(Arc::new(node));
        } else {
            next_offset.get_or_insert(seen - 1);
            overflow_count += 1;
            overflow_size += node.size;
            overflow_logical += node.logical_size.unwrap_or(node.size);
            overflow_unknown |= node.allocation_unknown;
        }
    }

    if run.is_cancelled() {
        lifecycle.finish(true);
        return Err(AppError::new("scanCancelled"));
    }
    if overflow_count > 0 {
        let mut overflow = ScanNode::overflow(overflow_count, overflow_size);
        overflow.logical_size = run.storage.as_ref().map(|_| overflow_logical);
        overflow.allocation_unknown = overflow_unknown;
        overflow.limit_reached = true;
        overflow.next_offset = next_offset;
        children.push(Arc::new(overflow));
    }

    // Every child is returned, unfolded: sort_and_cap would just rebuild the exact same
    // "N always-kept + overflow" split from the same data, handing back the same top items
    // and a same-sized new overflow node instead of anything new. The point of this command
    // is specifically to show what folding hid.
    children.sort_unstable_by_key(|node| Reverse((node.size, node.name.len())));
    lifecycle.finish(run.diagnostics.snapshot().coverage == "partial");
    Ok(children)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn precise_walk_deduplicates_hardlinks_across_folders_and_preserves_logical_sizes() {
        let root =
            std::env::temp_dir().join(format!("discreveal-precise-tree-{}", rand::random::<u64>()));
        struct OwnTree(PathBuf);
        impl Drop for OwnTree {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fs::create_dir(&root).unwrap();
        let _fixture = OwnTree(root.clone());
        let alpha = root.join("a.bin");
        fs::write(&alpha, vec![3; 8193]).unwrap();
        let folder = root.join("folder");
        fs::create_dir(&folder).unwrap();
        fs::hard_link(&alpha, folder.join("alias.bin")).unwrap();
        fs::write(folder.join("other.bin"), vec![5; 8193]).unwrap();
        let expected = crate::storage::measure(&alpha).unwrap().allocated
            + crate::storage::measure(&folder.join("other.bin"))
                .unwrap()
                .allocated;
        let mut precise = run();
        precise.storage = Some(Arc::new(crate::storage::Ledger::default()));
        let tree = walk(&root, "root", 0, &precise, &AtomicU64::new(0));
        assert_eq!(tree.size, expected);
        assert_eq!(tree.logical_size, Some(3 * 8193));
        let folder_node = tree
            .children
            .iter()
            .find(|node| node.name == "folder")
            .unwrap();
        let alias = folder_node
            .children
            .iter()
            .find(|node| node.name == "alias.bin")
            .unwrap();
        assert!(alias.shared_storage);
        assert_eq!(alias.size, 0);
        assert_eq!(alias.logical_size, Some(8193));
        assert_eq!(precise.diagnostics.snapshot().coverage, "complete");
        let reread = walk(&root, "root", 0, &precise, &AtomicU64::new(0));
        assert_eq!(reread.size, tree.size);
        let fast = walk(&root, "root", 0, &run(), &AtomicU64::new(0));
        assert_eq!(fast.size, 3 * 8193);
        assert_eq!(fast.logical_size, None);
    }

    #[cfg(windows)]
    #[test]
    fn precise_walk_reports_offline_allocation_instead_of_recalling_data_or_falling_back_to_length()
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, SetFileAttributesW,
        };
        let root =
            std::env::temp_dir().join(format!("discreveal-precise-lock-{}", rand::random::<u64>()));
        fs::create_dir(&root).unwrap();
        let path = root.join("locked.bin");
        fs::write(&path, vec![3; 8193]).unwrap();
        struct OwnTree(PathBuf);
        impl Drop for OwnTree {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _fixture = OwnTree(root.clone());
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: own isolated file and live NUL-terminated pathname.
        assert_ne!(
            unsafe { SetFileAttributesW(name.as_ptr(), FILE_ATTRIBUTE_OFFLINE) },
            0
        );
        let mut precise = run();
        precise.storage = Some(Arc::new(crate::storage::Ledger::default()));
        let tree = walk(&root, "root", 0, &precise, &AtomicU64::new(0));
        assert_eq!(tree.size, 0);
        assert_eq!(tree.logical_size, Some(8193));
        assert!(tree.allocation_unknown);
        assert!(tree.children[0].allocation_unknown);
        assert_eq!(precise.diagnostics.snapshot().coverage, "partial");
        // SAFETY: same owned fixture; clear its synthetic offline flag.
        assert_ne!(
            unsafe { SetFileAttributesW(name.as_ptr(), FILE_ATTRIBUTE_NORMAL) },
            0
        );
    }

    #[test]
    fn cancelling_before_root_listing_finishes_cancels_the_pending_scan() {
        let state = ScanState::new();
        let first = state.begin_scan();
        assert!(state.root().is_none());
        state.request_cancel();
        assert!(first.load(Ordering::Relaxed));
        let second = state.begin_scan();
        assert!(!second.load(Ordering::Relaxed));
        let third = state.begin_scan();
        assert!(second.load(Ordering::Relaxed));
        assert!(!third.load(Ordering::Relaxed));
    }

    #[test]
    fn depth_limit_has_an_explicit_node_flag_and_partial_coverage() {
        let run = run();
        let node = walk(
            Path::new(r"C:\never-read"),
            "deep",
            MAX_DEPTH + 1,
            &run,
            &AtomicU64::new(0),
        );
        assert!(node.limit_reached);
        assert!(!node.access_denied);
        assert_eq!(run.diagnostics.snapshot().depth_limited, 1);
        assert_eq!(run.diagnostics.snapshot().coverage, "partial");
    }

    fn run() -> ScanRun {
        ScanRun {
            storage: None,
            filter: ScanFilter {
                excludes_lower: Vec::new(),
                min_size_bytes: 0,
                max_size_bytes: 0,
            },
            started: Instant::now(),
            scanned_files: AtomicU64::new(0),
            scanned_bytes: AtomicU64::new(0),
            last_activity_ms: AtomicU64::new(0),
            current_dir: Mutex::new(String::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
            diagnostics: Diagnostics::default(),
            lifecycle: None,
        }
    }

    #[test]
    fn progress_flushes_small_and_cancelled_batches_exactly_once() {
        let run = run();
        let bytes = AtomicU64::new(0);
        {
            let mut progress = FileProgress::new(&run, &bytes);
            progress.record(17);
            progress.record(0);
            run.cancel();
        }
        assert_eq!(run.scanned_files.load(Ordering::Relaxed), 2);
        assert_eq!(run.scanned_bytes.load(Ordering::Relaxed), 17);
        assert_eq!(bytes.load(Ordering::Relaxed), 17);
    }

    #[test]
    fn progress_publishes_full_batches_and_slow_partial_batches() {
        let run = run();
        let bytes = AtomicU64::new(0);
        let mut progress = FileProgress::new(&run, &bytes);
        for _ in 0..64 {
            progress.record(3);
        }
        assert_eq!(run.scanned_files.load(Ordering::Relaxed), 64);
        progress.published_at = Instant::now() - Duration::from_millis(101);
        progress.record(5);
        assert_eq!(run.scanned_files.load(Ordering::Relaxed), 65);
        progress.flush();
        drop(progress);
        assert_eq!(bytes.load(Ordering::Relaxed), 197);
    }

    #[test]
    fn parallel_progress_preserves_all_files_and_bytes() {
        let run = run();
        let bytes = AtomicU64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    let mut progress = FileProgress::new(&run, &bytes);
                    for size in 0..1001 {
                        progress.record(size);
                    }
                });
            }
        });
        assert_eq!(run.scanned_files.load(Ordering::Relaxed), 8008);
        assert_eq!(run.scanned_bytes.load(Ordering::Relaxed), 4_004_000);
        assert_eq!(bytes.load(Ordering::Relaxed), 4_004_000);
    }

    #[test]
    fn partial_snapshots_reuse_sent_subtrees_but_done_tree_remains_complete() {
        let subtree = Arc::new(ScanNode::dir(
            "A",
            Path::new(r"C:\A"),
            5000,
            (0..5000).map(|i| node(&format!("file{i}"), 1)).collect(),
            false,
        ));
        let job = ScanJob {
            id: 1,
            run: run(),
            root: PathBuf::from(r"C:\"),
            root_name: "C:".into(),
            root_path: r"C:\".into(),
            root_files: Vec::new(),
            top_levels: vec![
                TopLevelDir {
                    path: PathBuf::from(r"C:\A"),
                    name: "A".into(),
                    bytes_so_far: AtomicU64::new(5000),
                    result: Mutex::new(Some(Arc::clone(&subtree))),
                },
                TopLevelDir {
                    path: PathBuf::from(r"C:\B"),
                    name: "B".into(),
                    bytes_so_far: AtomicU64::new(7),
                    result: Mutex::new(None),
                },
            ],
            worker_count: 6,
            done_emitted: AtomicBool::new(false),
            next_partial_at: Mutex::new(Instant::now()),
            sent_subtrees: Mutex::new(HashSet::new()),
        };
        let (first, reused, fresh) = job.assemble_partial();
        assert!(reused.is_empty());
        assert_eq!(fresh, vec![r"C:\A"]);
        // Failed delivery does not mark the tree sent: an immediate retry is full.
        assert_eq!(job.assemble_partial().2, fresh);
        lock(&job.sent_subtrees).extend(fresh);
        job.top_levels[1].bytes_so_far.store(11, Ordering::Relaxed);
        let (second, reused, fresh) = job.assemble_partial();
        assert_eq!(reused, vec![r"C:\A"]);
        assert!(fresh.is_empty());
        assert_eq!(second.size, 5011);
        assert!(second.children[0].children.is_empty());
        assert!(second.children[1].in_progress);
        assert!(
            serde_json::to_vec(&second).unwrap().len() * 100
                < serde_json::to_vec(&first).unwrap().len()
        );
        let full = job.assemble_root();
        assert_eq!(full.children[0].children.len(), 5000);
        assert!(Arc::ptr_eq(&full.children[0], &subtree));
        *lock(&job.top_levels[1].result) = Some(Arc::new(ScanNode::dir(
            "B",
            Path::new(r"C:\B"),
            11,
            vec![node("new", 11)],
            false,
        )));
        let (_, reused, fresh) = job.assemble_partial();
        assert_eq!(reused, vec![r"C:\A"]);
        assert_eq!(fresh, vec![r"C:\B"]);
    }

    fn node(name: &str, size: u64) -> Arc<ScanNode> {
        Arc::new(ScanNode::file(
            name.to_string(),
            format!("C:\\{name}"),
            size,
            0,
            false,
        ))
    }

    fn nodes(sizes: &[u64]) -> Vec<Arc<ScanNode>> {
        sizes
            .iter()
            .enumerate()
            .map(|(i, size)| node(&format!("n{i}"), *size))
            .collect()
    }

    #[test]
    fn large_directories_keep_the_same_top_children_as_a_full_sort_would() {
        // Regression test for the select_nth_unstable_by_key optimization in sort_and_cap:
        // a pseudo-random, non-sorted, all-distinct size list well beyond MAX_CHILDREN, checked
        // against what a plain full sort would have kept as the top ALWAYS_KEPT_CHILDREN.
        let mut sizes: Vec<u64> = (0..5000)
            .map(|i| (i * 2654435761u64) % 10_000_000)
            .collect();
        sizes.dedup();
        let total: u64 = sizes.iter().sum();

        let mut expected_sorted = sizes.clone();
        expected_sorted.sort_unstable_by_key(|s| Reverse(*s));
        let expected_top: Vec<u64> = expected_sorted
            .into_iter()
            .take(ALWAYS_KEPT_CHILDREN)
            .collect();

        let mut children = nodes(&sizes);
        sort_and_cap(&mut children, total);
        let actual_top: Vec<u64> = children
            .iter()
            .take(ALWAYS_KEPT_CHILDREN)
            .map(|n| n.size)
            .collect();

        assert_eq!(actual_top, expected_top);
        assert!(children.len() <= MAX_CHILDREN + 1);
    }

    #[test]
    fn small_directories_are_left_untouched() {
        let mut children = nodes(&[5, 9, 1]);
        sort_and_cap(&mut children, 15);
        assert_eq!(
            children.iter().map(|n| n.size).collect::<Vec<_>>(),
            vec![9, 5, 1]
        );
    }

    #[test]
    fn insignificant_children_are_folded_without_changing_the_total() {
        let mut sizes = vec![1_000_000u64; 5];
        sizes.extend(vec![1u64; 100]);
        let total: u64 = sizes.iter().sum();
        let mut children = nodes(&sizes);
        sort_and_cap(&mut children, total);

        assert_eq!(children.len(), ALWAYS_KEPT_CHILDREN + 1);
        assert_eq!(children.iter().map(|n| n.size).sum::<u64>(), total);
        assert!(children.last().unwrap().path.is_empty());
        assert_eq!(
            children.last().unwrap().overflow_count,
            Some(105 - ALWAYS_KEPT_CHILDREN)
        );
    }

    #[test]
    fn a_single_surplus_child_is_not_folded() {
        let mut sizes = vec![1_000_000u64; ALWAYS_KEPT_CHILDREN];
        sizes.push(1);
        let total: u64 = sizes.iter().sum();
        let mut children = nodes(&sizes);
        sort_and_cap(&mut children, total);
        assert_eq!(children.len(), ALWAYS_KEPT_CHILDREN + 1);
        assert!(!children.last().unwrap().path.is_empty());
    }

    #[test]
    fn child_count_never_exceeds_the_cap() {
        let equal = vec![1_000u64; 1_000];
        let geometric: Vec<u64> = (0..200).map(|i| 1_000_000 / (i + 1)).collect();
        for sizes in [equal, geometric] {
            let total: u64 = sizes.iter().sum();
            let mut children = nodes(&sizes);
            sort_and_cap(&mut children, total);
            assert!(children.len() <= MAX_CHILDREN + 1);
            assert_eq!(children.iter().map(|n| n.size).sum::<u64>(), total);
        }
    }

    #[test]
    fn excludes_match_whole_path_components() {
        assert!(mark_markinson_contains_at_boundaries(
            r"d:\data\file.txt",
            r"d:\data"
        ));
        assert!(mark_markinson_contains_at_boundaries(
            r"d:\data", r"d:\data"
        ));
        assert!(!mark_markinson_contains_at_boundaries(
            r"d:\database\file.txt",
            r"d:\data"
        ));
        assert!(!mark_markinson_contains_at_boundaries(
            r"d:\data2",
            r"d:\data"
        ));
        assert!(mark_markinson_contains_at_boundaries(
            r"c:\projects\app\node_modules\x",
            "node_modules"
        ));
        assert!(!mark_markinson_contains_at_boundaries(
            r"c:\projects\my_node_modules\x",
            "node_modules"
        ));
        assert!(mark_markinson_contains_at_boundaries(
            r"d:\steamlibrary\steamapps\common",
            r"\steamapps"
        ));
        assert!(!mark_markinson_contains_at_boundaries(
            r"d:\idea games\x",
            r"\ea games"
        ));
        assert!(mark_markinson_contains_at_boundaries(
            r"c:\program files\epic games\launcher",
            r"\epic games"
        ));
    }

    #[test]
    fn is_excluded_is_case_insensitive_on_ascii_paths() {
        let excludes = vec!["\\steamapps".to_string(), "node_modules".to_string()];
        assert!(is_excluded(
            Path::new(r"D:\SteamLibrary\STEAMAPPS\common"),
            &excludes
        ));
        assert!(is_excluded(
            Path::new(r"C:\projects\app\node_modules\x"),
            &excludes
        ));
        assert!(!is_excluded(Path::new(r"D:\database\file.txt"), &excludes));
        assert!(!is_excluded(
            Path::new(r"C:\projects\my_node_modules\x"),
            &excludes
        ));
        assert!(!is_excluded(Path::new(r"C:\Users\x"), &[]));
    }

    #[test]
    fn is_excluded_handles_non_ascii_paths_too() {
        let excludes = vec!["ä".to_string()];
        assert!(is_excluded(Path::new("d:\\ä\\x"), &excludes));
        assert!(!is_excluded(Path::new("d:\\ää\\x"), &excludes));
    }

    #[test]
    fn directory_filters_match_full_path_rules_including_unicode_and_trailing_separators() {
        let directories = [
            r"C:\",
            r"\\?\C:\",
            r"C:\Users\Alice",
            r"C:\Users\Alice\node_modules",
            r"D:\Data",
            r"D:\Database",
            r"D:\SteamLibrary\steamapps",
            r"C:\Ubisoft Game Launcher",
            r"C:\Äpfel",
            r"C:\ä",
            r"C:\ää",
            r"C:\Foo\Bar",
        ];
        let names = [
            "file.txt",
            "node_modules",
            "my_node_modules",
            "SteamApps",
            "data",
            "database",
            "games",
            "Ä",
            "ää",
            "日本語",
            "foo",
            "bar",
            "baz",
            "K",
            "İ",
        ];
        let fragments = [
            r"\steamapps",
            "node_modules",
            r"d:\data",
            r"d:\data\",
            "ä",
            "ää",
            r"\ubisoft game launcher\games",
            r"c:\users\alice",
            r"c:\users\alice\",
            r"\foo\bar",
            r"foo\bar\baz",
            r"foo\bar\",
            "file.txt",
            "日本語",
            "k",
            "i\u{307}",
            "\\",
            r"\\?\c:\",
        ];
        for directory in directories {
            for fragment in fragments {
                let excludes_lower = vec![fragment.to_lowercase()];
                let filter = ScanFilter {
                    excludes_lower: excludes_lower.clone(),
                    min_size_bytes: 0,
                    max_size_bytes: 0,
                };
                let compiled = filter.for_directory(Path::new(directory));
                for name in names {
                    let path = Path::new(directory).join(name);
                    assert_eq!(
                        compiled.is_excluded(name),
                        is_excluded(&path, &excludes_lower),
                        "directory={directory:?} name={name:?} fragment={fragment:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn boundary_search_handles_multibyte_paths() {
        assert!(!mark_markinson_contains_at_boundaries("d:\\ää\\x", "ä"));
        assert!(mark_markinson_contains_at_boundaries("d:\\ä\\x", "ä"));
    }

    #[test]
    fn only_drive_letter_paths_are_scannable() {
        assert!(is_local_drive_path(Path::new(r"C:\Users")));
        assert!(is_local_drive_path(Path::new(r"\\?\C:\Users")));
        assert!(!is_local_drive_path(Path::new(r"\\server\share")));
        assert!(!is_local_drive_path(Path::new(r"\\?\UNC\server\share")));
        assert!(!is_local_drive_path(Path::new(r"\\.\PhysicalDrive0")));
    }

    #[test]
    fn limits_reject_invalid_values() {
        assert_eq!(limit_to_bytes(0.0, 1024.0), 0);
        assert_eq!(limit_to_bytes(-5.0, 1024.0), 0);
        assert_eq!(limit_to_bytes(f64::NAN, 1024.0), 0);
        assert_eq!(limit_to_bytes(f64::INFINITY, 1024.0), 0);
        assert_eq!(limit_to_bytes(2.0, 1024.0), 2048);
    }

    #[test]
    fn worker_count_follows_the_medium() {
        assert_eq!(worker_count_for_drive("NVMe", true), NVME_WORKERS);
        assert_eq!(worker_count_for_drive("SATA", true), SSD_WORKERS);
        assert_eq!(worker_count_for_drive("USB", true), USB_WORKERS);
        assert_eq!(worker_count_for_drive("SATA", false), HDD_WORKERS);
    }

    #[test]
    fn size_filter_bounds() {
        let filter = ScanFilter {
            excludes_lower: Vec::new(),
            min_size_bytes: 100,
            max_size_bytes: 1000,
        };
        assert!(filter.is_out_of_range(99));
        assert!(!filter.is_out_of_range(100));
        assert!(!filter.is_out_of_range(1000));
        assert!(filter.is_out_of_range(1001));
        let unlimited = ScanFilter {
            excludes_lower: Vec::new(),
            min_size_bytes: 0,
            max_size_bytes: 0,
        };
        assert!(!unlimited.is_out_of_range(0));
        assert!(!unlimited.is_out_of_range(u64::MAX));
    }
}
