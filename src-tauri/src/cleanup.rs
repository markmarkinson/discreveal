//! Curated, memory-only cleanup plans. The renderer can select registered
//! groups, never arbitrary paths. All mutations recheck the original identity.
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;

#[path = "cleanup_catalog.rs"]
mod catalog;
#[path = "cleanup_system.rs"]
pub(crate) mod system;

const MAX_ITEMS: usize = 100_000;
const MAX_DEPTH: usize = 64;
const DAY: u64 = 86_400;
const BLOCKED_ATTRIBUTES: u32 =
    FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_READONLY;

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
pub struct CleanupState {
    inner: Arc<Mutex<Inner>>,
    sequence: AtomicU64,
}
#[derive(Default)]
struct Inner {
    busy: bool,
    cancel: Option<Arc<AtomicBool>>,
    plan: Option<Plan>,
    additional_rules: Vec<Rule>,
    system_plan: Option<system::SystemPlan>,
}
struct Operation {
    inner: Arc<Mutex<Inner>>,
    lifecycle: crate::operations::Operation,
    _lifecycle_guard: crate::operations::OperationGuard,
}
impl Operation {
    fn attach(&self, app: &AppHandle) {
        crate::operation_ipc::attach(&self.lifecycle, app);
    }
    fn finish(&self, incomplete: bool) {
        self.lifecycle.finish(incomplete);
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        let mut state = lock(&self.inner);
        state.busy = false;
        state.cancel = None;
    }
}
impl CleanupState {
    #[cfg(test)]
    fn begin(&self) -> AppResult<(Operation, Arc<AtomicBool>)> {
        self.begin_kind("cleanup")
    }
    fn begin_kind(&self, kind: &'static str) -> AppResult<(Operation, Arc<AtomicBool>)> {
        let mut state = lock(&self.inner);
        if state.busy {
            return Err(AppError::new("cleanupBusy"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let lifecycle = crate::operations::Operation::start(kind, None, cancel.clone())?;
        let lifecycle_guard = lifecycle.guard();
        state.busy = true;
        state.cancel = Some(cancel.clone());
        Ok((
            Operation {
                inner: self.inner.clone(),
                lifecycle,
                _lifecycle_guard: lifecycle_guard,
            },
            cancel,
        ))
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    id: String,
    label: String,
    effect: String,
    recommended: bool,
    available: bool,
    blocked: bool,
    min_days: u32,
    category: String,
    processes: Vec<String>,
    #[serde(skip)]
    log_only: bool,
    #[serde(skip)]
    roots: Vec<PathBuf>,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Identity {
    volume: u64,
    file_id: [u8; 16],
    size: u64,
    modified: u64,
}
#[derive(Clone)]
struct Candidate {
    id: u64,
    group: String,
    path: PathBuf,
    root: PathBuf,
    identity: Identity,
    allocated: u64,
}
#[derive(Clone)]
struct Plan {
    id: u64,
    rules: Vec<Rule>,
    files: Vec<Candidate>,
}

fn complete_cache_plan(inner: &Mutex<Inner>, remaining: Plan) {
    // The other part of a confirmed batch must survive this transition.
    lock(inner).plan = Some(remaining);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisOptions {
    rule_ids: Vec<String>,
    min_days: u32,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    id: String,
    count: usize,
    size: u64,
    allocated: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    plan_id: u64,
    groups: Vec<Group>,
    skipped: u64,
    cancelled: bool,
    truncated: bool,
    elapsed_ms: u64,
    limited_groups: Vec<String>,
    issues: HashMap<String, u64>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    plan_id: u64,
    phase: &'static str,
    groups: Vec<Group>,
    checked: u64,
    skipped: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    id: u64,
    path: String,
    size: u64,
    allocated: u64,
    modified: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    removed: u64,
    size: u64,
    estimated_freed: u64,
    observed_free_change: Option<i64>,
    skipped: u64,
    cancelled: bool,
    remaining: Vec<Group>,
    reasons: HashMap<String, u64>,
}

fn known_folder(folder: &windows_sys::core::GUID) -> io::Result<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
    let mut raw = std::ptr::null_mut();
    // SAFETY: Shell allocates a null-terminated string; released with its matching allocator.
    let status = unsafe { SHGetKnownFolderPath(folder, 0, std::ptr::null_mut(), &mut raw) };
    if status < 0 || raw.is_null() {
        return Err(io::Error::other("LocalAppData unavailable"));
    }
    let result = unsafe {
        let mut length = 0;
        while *raw.add(length) != 0 {
            length += 1;
        }
        use std::os::windows::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_wide(std::slice::from_raw_parts(
            raw, length,
        )));
        CoTaskMemFree(raw.cast());
        path
    };
    Ok(result)
}

fn processes() -> io::Result<HashSet<String>> {
    // SAFETY: valid snapshot flags; owned snapshot is closed on every return path.
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut found = HashSet::new();
    let mut ok = unsafe { Process32FirstW(handle, &mut entry) };
    if ok == 0 {
        unsafe { CloseHandle(handle) };
        return Err(io::Error::last_os_error());
    }
    while ok != 0 {
        let length = entry
            .szExeFile
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(entry.szExeFile.len());
        found.insert(String::from_utf16_lossy(&entry.szExeFile[..length]).to_lowercase());
        ok = unsafe { Process32NextW(handle, &mut entry) };
    }
    unsafe { CloseHandle(handle) };
    Ok(found)
}

fn ordinary_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| {
        meta.is_dir()
            && meta.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE) == 0
    })
}
fn browser_roots(base: &Path, firefox: bool) -> Vec<PathBuf> {
    if !ordinary_directory(base) {
        return vec![];
    }
    let Ok(entries) = fs::read_dir(base) else {
        return vec![];
    };
    // Only rebuildable cache/download directories, never installed components,
    // extensions, service workers, account state or offline site data.
    let mut roots: Vec<PathBuf> = if firefox {
        Vec::new()
    } else {
        [
            "component_crx_cache",
            "extensions_crx_cache",
            "ShaderCache",
            "GrShaderCache",
            "GraphiteDawnCache",
        ]
        .iter()
        .map(|name| base.join(name))
        .collect()
    };
    for entry in entries.flatten().take(500) {
        let path = entry.path();
        if !ordinary_directory(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if firefox {
            roots.push(path.join("cache2"));
        } else if name == "Default" || name.starts_with("Profile ") {
            roots.push(path.join("Cache"));
            roots.push(path.join("Code Cache"));
            for name in [
                "GPUCache",
                "DawnCache",
                "DawnGraphiteCache",
                "DawnWebGPUCache",
                "Media Cache",
            ] {
                roots.push(path.join(name));
            }
        }
    }
    roots
        .into_iter()
        .filter(|path| ordinary_directory(path))
        .collect()
}
fn discover_rules() -> AppResult<Vec<Rule>> {
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_ProgramData, FOLDERID_RoamingAppData,
    };
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    let program_data = known_folder(&FOLDERID_ProgramData)?;
    Ok(catalog::discover(
        &local,
        &roaming,
        &program_data,
        processes().ok().as_ref(),
    ))
}

fn valid_rule_ids(ids: &[String], rules: &[Rule]) -> bool {
    !ids.is_empty()
        && ids.len() <= rules.len()
        && ids.iter().collect::<HashSet<_>>().len() == ids.len()
        && ids.iter().all(|id| rules.iter().any(|rule| &rule.id == id))
}

fn blocked_processes(names: &[String], running: Option<&HashSet<String>>) -> bool {
    !names.is_empty() && running.is_none_or(|set| names.iter().any(|name| set.contains(name)))
}

#[tauri::command(async)]
pub fn cleanup_rules(state: State<'_, CleanupState>) -> AppResult<Vec<Rule>> {
    let mut rules = discover_rules()?;
    let running = processes().ok();
    rules.extend(
        lock(&state.inner)
            .additional_rules
            .iter()
            .cloned()
            .map(|mut rule| {
                rule.blocked = blocked_processes(&rule.processes, running.as_ref());
                rule.available = rule.roots.iter().any(|path| ordinary_directory(path));
                rule
            }),
    );
    Ok(rules)
}

fn snapshot(file: &File) -> io::Result<(Identity, u64)> {
    let handle = file.as_raw_handle() as HANDLE;
    let mut basic: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let mut id: FILE_ID_INFO = unsafe { std::mem::zeroed() };
    let mut standard: FILE_STANDARD_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: live handle and writable structures matching the information classes.
    let ok = unsafe {
        GetFileInformationByHandle(handle, &mut basic) != 0
            && GetFileInformationByHandleEx(
                handle,
                FileIdInfo,
                (&mut id as *mut FILE_ID_INFO).cast(),
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            ) != 0
            && GetFileInformationByHandleEx(
                handle,
                FileStandardInfo,
                (&mut standard as *mut FILE_STANDARD_INFO).cast(),
                std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
            ) != 0
    };
    if !ok {
        return Err(io::Error::last_os_error());
    }
    if basic.dwFileAttributes & (BLOCKED_ATTRIBUTES | FILE_ATTRIBUTE_DIRECTORY) != 0
        || basic.nNumberOfLinks != 1
        || standard.DeletePending
        || id.FileId.Identifier == [0; 16]
    {
        return Err(io::Error::other("unsupported file"));
    }
    Ok((
        Identity {
            volume: id.VolumeSerialNumber,
            file_id: id.FileId.Identifier,
            size: ((basic.nFileSizeHigh as u64) << 32) | basic.nFileSizeLow as u64,
            modified: ((basic.ftLastWriteTime.dwHighDateTime as u64) << 32)
                | basic.ftLastWriteTime.dwLowDateTime as u64,
        },
        standard.AllocationSize.max(0) as u64,
    ))
}
fn metadata_file(path: &Path, deleting: bool) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES | if deleting { DELETE } else { 0 })
        .share_mode(if deleting {
            FILE_SHARE_READ
        } else {
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}
fn old_enough(stamp: u64, now: u64, days: u32) -> bool {
    let seconds = stamp / 10_000_000;
    let modified = seconds.checked_sub(11_644_473_600);
    modified.is_some_and(|value| value <= now && now - value >= days as u64 * DAY)
}
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut bytes = 0;
    // SAFETY: null-terminated path and a writable integer output.
    (unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut bytes,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } != 0)
        .then_some(bytes)
}

fn groups(plan: &Plan) -> Vec<Group> {
    let mut values: HashMap<&str, Group> = HashMap::new();
    for file in &plan.files {
        let entry = values.entry(&file.group).or_insert_with(|| Group {
            id: file.group.clone(),
            count: 0,
            size: 0,
            allocated: 0,
        });
        entry.count += 1;
        entry.size += file.identity.size;
        entry.allocated += file.allocated;
    }
    plan.rules
        .iter()
        .filter_map(|rule| values.remove(rule.id.as_str()))
        .collect()
}

fn analyze(
    rules: Vec<Rule>,
    options: AnalysisOptions,
    id: u64,
    cancel: &AtomicBool,
    progress: impl FnMut(Progress),
) -> AnalysisResultAndPlan {
    analyze_with_budget(rules, options, id, cancel, MAX_ITEMS, progress)
}

fn analyze_with_budget(
    rules: Vec<Rule>,
    options: AnalysisOptions,
    id: u64,
    cancel: &AtomicBool,
    max_items: usize,
    mut progress: impl FnMut(Progress),
) -> AnalysisResultAndPlan {
    let start = Instant::now();
    let mut tick = Instant::now();
    let mut plan = Plan {
        id,
        rules,
        files: Vec::new(),
    };
    let mut skipped = 0;
    let mut checked = 0;
    let mut truncated = false;
    let mut seen = HashSet::new();
    let selected = plan
        .rules
        .iter()
        .filter(|rule| options.rule_ids.contains(&rule.id) && !rule.blocked)
        .count()
        .max(1);
    let per_group_limit = (max_items / selected).max(1);
    let mut limited_groups = Vec::new();
    let mut issues = HashMap::new();
    let now = now_seconds();
    let mut pending = HashSet::new();
    for pass in 0..2 {
        'rules: for rule in &plan.rules {
            if !options.rule_ids.contains(&rule.id) || rule.blocked {
                continue;
            }
            if pass == 1 && !pending.contains(&rule.id) {
                continue;
            }
            let limit = if pass == 0 {
                per_group_limit
            } else {
                max_items.saturating_sub(plan.files.len())
            };
            if limit == 0 {
                limited_groups.push(rule.id.clone());
                truncated = true;
                continue;
            }
            let skipped_before = skipped;
            let mut group_count = 0;
            'roots: for root in &rule.roots {
                // Validate every ancestor before touching a provider's root.
                if !safe_ancestors(root) || !ordinary_directory(root) {
                    skipped += 1;
                    continue;
                }
                let mut stack = vec![(root.clone(), 0usize)];
                while let Some((directory, depth)) = stack.pop() {
                    if cancel.load(Ordering::Relaxed) {
                        break 'rules;
                    }
                    if depth >= MAX_DEPTH {
                        skipped += 1;
                        continue;
                    }
                    let Ok(entries) = crate::scan_directory::read_dir(&directory) else {
                        skipped += 1;
                        continue;
                    };
                    for entry in entries {
                        if cancel.load(Ordering::Relaxed) {
                            break 'rules;
                        }
                        let Ok(entry) = entry else {
                            skipped += 1;
                            continue;
                        };
                        let path = entry.path();
                        checked += 1;
                        if tick.elapsed() >= Duration::from_millis(150) {
                            progress(Progress {
                                plan_id: id,
                                phase: "analyze",
                                groups: groups(&plan),
                                checked,
                                skipped,
                            });
                            tick = Instant::now();
                        }
                        let Ok((directory, attributes, modified)) = entry.cleanup_details() else {
                            skipped += 1;
                            continue;
                        };
                        if attributes & BLOCKED_ATTRIBUTES != 0 {
                            skipped += 1;
                            continue;
                        }
                        if directory {
                            stack.push((path, depth + 1));
                            continue;
                        }
                        if rule.log_only
                            && !path
                                .extension()
                                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                        {
                            continue;
                        }
                        if !old_enough(modified, now, options.min_days.max(rule.min_days)) {
                            continue;
                        }
                        let Ok(file) = metadata_file(&path, false) else {
                            skipped += 1;
                            continue;
                        };
                        let Ok((identity, allocated)) = snapshot(&file) else {
                            skipped += 1;
                            continue;
                        };
                        if !old_enough(identity.modified, now, options.min_days.max(rule.min_days))
                        {
                            continue;
                        }
                        if seen.insert((identity.volume, identity.file_id)) {
                            group_count += 1;
                            plan.files.push(Candidate {
                                id: plan.files.len() as u64 + 1,
                                group: rule.id.clone(),
                                path,
                                root: root.clone(),
                                identity,
                                allocated,
                            });
                        }
                        if group_count >= limit {
                            if pass == 0 {
                                pending.insert(rule.id.clone());
                            } else {
                                truncated = true;
                                limited_groups.push(rule.id.clone());
                            }
                            break 'roots;
                        }
                    }
                }
            }
            if skipped > skipped_before {
                issues.insert(rule.id.clone(), skipped - skipped_before);
            }
        }
    }
    let result = AnalysisResult {
        plan_id: id,
        groups: groups(&plan),
        skipped,
        cancelled: cancel.load(Ordering::Relaxed),
        truncated,
        elapsed_ms: start.elapsed().as_millis() as u64,
        limited_groups,
        issues,
    };
    AnalysisResultAndPlan(result, plan)
}
struct AnalysisResultAndPlan(AnalysisResult, Plan);

fn safe_ancestors(path: &Path) -> bool {
    path.is_absolute()
        && path
            .ancestors()
            .all(|ancestor| ancestor.as_os_str().is_empty() || ordinary_directory(ancestor))
}

#[tauri::command]
pub async fn analyze_cleanup(
    app: AppHandle,
    state: State<'_, CleanupState>,
    options: AnalysisOptions,
) -> AppResult<AnalysisResult> {
    if options.min_days > 3650 {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let (operation, cancel) = state.begin_kind("cleanupAnalyze")?;
    operation.attach(&app);
    let mut rules = discover_rules()?;
    let running = processes().ok();
    rules.extend(
        lock(&state.inner)
            .additional_rules
            .iter()
            .cloned()
            .map(|mut rule| {
                rule.blocked = blocked_processes(&rule.processes, running.as_ref());
                rule.available = rule.roots.iter().any(|path| ordinary_directory(path));
                rule
            }),
    );
    if !valid_rule_ids(&options.rule_ids, &rules) {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let id = state.sequence.fetch_add(1, Ordering::Relaxed) + 1;
    let inner = state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let AnalysisResultAndPlan(result, plan) = analyze(rules, options, id, &cancel, |payload| {
            let _ = app.emit("cleanup:progress", payload);
        });
        lock(&inner).plan = Some(plan);
        _operation.finish(result.cancelled || result.truncated || result.skipped > 0);
        Ok(result)
    })
    .await
    .map_err(|error| AppError::with_detail("cleanupFailed", error))?
}

/// Hold every ancestor without DELETE sharing while removing a file. This
/// prevents a directory/junction swap after validation. Never follows reparse points.
fn hold_ancestors(path: &Path) -> io::Result<Vec<File>> {
    let mut parents: Vec<_> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    parents.reverse();
    let mut held = Vec::new();
    for parent in parents {
        // Attribute-only handles do not reliably enforce sharing restrictions.
        // LIST_DIRECTORY makes a concurrent parent rename fail while held.
        let file = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(parent)?;
        let meta = file.metadata()?;
        if !meta.is_dir()
            || meta.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE) != 0
        {
            return Err(io::Error::other("unsafe parent"));
        }
        held.push(file);
    }
    Ok(held)
}

#[cfg(test)]
fn remove_candidate(candidate: &Candidate) -> Result<(), &'static str> {
    remove_with_parent_guard(candidate, &mut None)
}

type ParentGuard = Option<(PathBuf, Vec<File>)>;
fn remove_with_parent_guard(
    candidate: &Candidate,
    held: &mut ParentGuard,
) -> Result<(), &'static str> {
    if !candidate.path.starts_with(&candidate.root) || candidate.path == candidate.root {
        return Err("unsafe");
    }
    let parent = candidate.path.parent().ok_or("unsafe")?;
    if held.as_ref().is_none_or(|(path, _)| path != parent) {
        let guards = hold_ancestors(parent).map_err(|_| "unsafe")?;
        *held = Some((parent.to_path_buf(), guards));
    }
    let file = metadata_file(&candidate.path, true).map_err(|_| "inUse")?;
    let (identity, _) = snapshot(&file).map_err(|_| "unsafe")?;
    if identity != candidate.identity {
        return Err("changed");
    }
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the checked file handle has DELETE access and stays open until this call completes.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err("inUse");
    }
    drop(file);
    Ok(())
}

#[tauri::command]
pub async fn execute_cleanup(
    app: AppHandle,
    state: State<'_, CleanupState>,
    plan_id: u64,
    group_ids: Vec<String>,
    excluded_ids: Vec<u64>,
    acknowledged: bool,
) -> AppResult<ExecutionResult> {
    if !acknowledged {
        return Err(AppError::new("cleanupNotAcknowledged"));
    }
    let (operation, cancel) = state.begin_kind("cleanupExecute")?;
    operation.attach(&app);
    let plan = lock(&state.inner)
        .plan
        .clone()
        .filter(|plan| plan.id == plan_id)
        .ok_or_else(|| AppError::new("cleanupStalePlan"))?;
    if !valid_rule_ids(&group_ids, &plan.rules) {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    if excluded_ids.len() > MAX_ITEMS {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let excluded: HashSet<_> = excluded_ids.into_iter().collect();
    let inner = state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut result = ExecutionResult {
            removed: 0,
            size: 0,
            estimated_freed: 0,
            observed_free_change: None,
            skipped: 0,
            cancelled: false,
            remaining: vec![],
            reasons: HashMap::new(),
        };
        // Track each selected file volume once, including redirected cache locations.
        let mut volume_paths = HashMap::new();
        for candidate in plan
            .files
            .iter()
            .filter(|file| group_ids.contains(&file.group) && !excluded.contains(&file.id))
        {
            volume_paths
                .entry(candidate.identity.volume)
                .or_insert(candidate.root.clone());
        }
        let before: HashMap<_, _> = volume_paths
            .iter()
            .filter_map(|(volume, path)| free_space(path).map(|bytes| (*volume, bytes)))
            .collect();
        let mut deleted = HashSet::new();
        let mut tick = Instant::now();
        let mut parent_guard = None;
        let mut process_tick = Instant::now();
        let mut running = processes().ok();
        for candidate in plan
            .files
            .iter()
            .filter(|file| group_ids.contains(&file.group) && !excluded.contains(&file.id))
        {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
            if process_tick.elapsed() > Duration::from_millis(250) {
                running = processes().ok();
                process_tick = Instant::now();
            }
            let rule = plan
                .rules
                .iter()
                .find(|rule| rule.id == candidate.group)
                .ok_or_else(|| AppError::new("cleanupStalePlan"))?;
            let blocked = blocked_processes(&rule.processes, running.as_ref());
            let removed = if blocked {
                Err("appRunning")
            } else {
                remove_with_parent_guard(candidate, &mut parent_guard)
            };
            match removed {
                Ok(()) => {
                    result.removed += 1;
                    result.size += candidate.identity.size;
                    result.estimated_freed += candidate.allocated;
                    deleted.insert(candidate.id);
                }
                Err(reason) => {
                    result.skipped += 1;
                    *result.reasons.entry(reason.into()).or_default() += 1;
                }
            }
            if tick.elapsed() >= Duration::from_millis(150) {
                let _ = app.emit(
                    "cleanup:progress",
                    Progress {
                        plan_id,
                        phase: "clean",
                        groups: vec![],
                        checked: result.removed + result.skipped,
                        skipped: result.skipped,
                    },
                );
                tick = Instant::now();
            }
        }
        let changes: Option<Vec<i128>> = volume_paths
            .iter()
            .map(|(volume, path)| {
                before
                    .get(volume)
                    .zip(free_space(path))
                    .map(|(before, after)| after as i128 - *before as i128)
            })
            .collect();
        result.observed_free_change = changes.filter(|values| !values.is_empty()).map(|values| {
            values
                .iter()
                .sum::<i128>()
                .clamp(i64::MIN as i128, i64::MAX as i128) as i64
        });
        let mut remaining = plan;
        remaining.files.retain(|file| !deleted.contains(&file.id));
        result.remaining = groups(&remaining);
        // Cache and Windows plans are independent parts of one confirmed operation.
        // Their own identity/fresh-scan checks protect against changes between steps.
        complete_cache_plan(&inner, remaining);
        _operation.finish(result.cancelled || result.skipped > 0);
        Ok(result)
    })
    .await
    .map_err(|error| AppError::with_detail("cleanupFailed", error))?
}

#[tauri::command]
pub fn cancel_cleanup(state: State<CleanupState>) {
    if let Some(cancel) = &lock(&state.inner).cancel {
        crate::operations::cancellation_requested(cancel);
    }
}

#[tauri::command]
pub fn cleanup_items(
    state: State<CleanupState>,
    plan_id: u64,
    group_id: String,
    offset: usize,
) -> AppResult<Vec<Item>> {
    let inner = lock(&state.inner);
    if inner.busy {
        return Err(AppError::new("cleanupBusy"));
    }
    let plan = inner
        .plan
        .as_ref()
        .filter(|plan| plan.id == plan_id)
        .ok_or_else(|| AppError::new("cleanupStalePlan"))?;
    Ok(plan
        .files
        .iter()
        .filter(|file| file.group == group_id)
        .skip(offset)
        .take(100)
        .map(|file| Item {
            id: file.id,
            path: file.path.to_string_lossy().into(),
            size: file.identity.size,
            allocated: file.allocated,
            modified: file.identity.modified / 10_000_000 - 11_644_473_600,
        })
        .collect())
}

/// Fixed local Windows settings target; no frontend-supplied URL or command.
#[tauri::command(async)]
pub fn open_cleanup_settings(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url("ms-settings:storagesense", None::<&str>)
        .map_err(|error| AppError::with_detail("openFailed", error))
}

fn windows_cleanup_args(drive: &str) -> Option<[String; 2]> {
    let bytes = drive.as_bytes();
    (bytes.len() == 3 && bytes[0].is_ascii_uppercase() && bytes[1] == b':' && bytes[2] == b'\\')
        .then(|| ["/d".into(), drive[..1].into()])
}

/// Launch the Windows-owned selection dialog for a fixed local drive. No silent
/// deletion, registry presets, service changes or frontend-supplied commands.
#[tauri::command]
pub async fn run_windows_cleanup(
    app: AppHandle,
    state: State<'_, CleanupState>,
    drive: String,
) -> AppResult<()> {
    if windows_cleanup_args(&drive).is_none() || !crate::drives::is_fixed_drive(Path::new(&drive)) {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let binary = catalog::windows_directory(true)?.join("cleanmgr.exe");
    if !binary.is_file() {
        return Err(AppError::new("cleanupNativeUnavailable"));
    }
    let (operation, _) = state.begin_kind("cleanupWindowsSettings")?;
    operation.attach(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        // GUI executable: Windows owns category selection, elevation and cancellation.
        let status = std::process::Command::new(&binary)
            .current_dir(
                binary
                    .parent()
                    .ok_or_else(|| AppError::new("cleanupFailed"))?,
            )
            .args(
                windows_cleanup_args(&drive)
                    .ok_or_else(|| AppError::new("cleanupInvalidOptions"))?,
            )
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(AppError::with_detail(
                "cleanupFailed",
                format!("Windows exit code {:?}", status.code()),
            ));
        }
        _operation.finish(false);
        Ok(())
    })
    .await
    .map_err(|error| AppError::with_detail("cleanupFailed", error))?
}

fn cargo_build_rule(project: &Path) -> AppResult<Rule> {
    if !safe_ancestors(project) || !project.join("Cargo.toml").is_file() {
        return Err(AppError::new("cleanupInvalidProject"));
    }
    let target = project.join("target");
    let tag = fs::read_to_string(target.join("CACHEDIR.TAG")).unwrap_or_default();
    if !tag
        .lines()
        .any(|line| line == "Signature: 8a477f597d28d172789f06886806bc55")
    {
        return Err(AppError::new("cleanupInvalidProject"));
    }
    // Only Cargo's standard intermediate directories, never the project, User
    // data, final binaries or an arbitrary external CARGO_TARGET_DIR.
    let target = &target;
    let roots = ["debug", "release"]
        .iter()
        .flat_map(|profile| {
            ["build", "deps", "incremental", ".fingerprint"]
                .iter()
                .map(move |part| target.join(profile).join(part))
        })
        .filter(|path| ordinary_directory(path))
        .collect::<Vec<_>>();
    let names = vec![
        "cargo.exe".into(),
        "rustc.exe".into(),
        "rust-analyzer.exe".into(),
    ];
    Ok(Rule {
        id: "cargo-build".into(),
        label: "cleanupCargoBuild".into(),
        category: "development".into(),
        effect: "cleanupBuildEffect".into(),
        available: !roots.is_empty(),
        blocked: blocked_processes(&names, processes().ok().as_ref()),
        roots,
        processes: names,
        recommended: false,
        min_days: 7,
        log_only: false,
    })
}

#[tauri::command(async)]
pub fn pick_cleanup_project(
    app: AppHandle,
    state: State<'_, CleanupState>,
    title: String,
) -> AppResult<bool> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let (_operation, _) = state.begin_kind("cleanupProject")?;
    _operation.attach(&app);
    let mut dialog = app.dialog().file().set_title(
        title
            .chars()
            .filter(|ch| !ch.is_control())
            .take(100)
            .collect::<String>(),
    );
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(folder) = dialog
        .blocking_pick_folder()
        .and_then(|folder| folder.into_path().ok())
    else {
        _operation.finish(false);
        return Ok(false);
    };
    let rule = cargo_build_rule(&folder)?;
    let mut inner = lock(&state.inner);
    inner.plan = None;
    inner.additional_rules = vec![rule];
    _operation.finish(false);
    Ok(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBin {
    drive: String,
    count: u64,
    size: u64,
}

fn query_recycle_bins() -> Vec<RecycleBin> {
    use windows_sys::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    let mut bins = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let wide = crate::win::wide(&drive);
        // Only explicit local fixed drives; no all-drive deletion through NULL.
        if !crate::drives::is_fixed_drive(Path::new(&drive)) {
            continue;
        }
        let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
        if unsafe { SHQueryRecycleBinW(wide.as_ptr(), &mut info) } >= 0 {
            bins.push(RecycleBin {
                drive,
                count: info.i64NumItems.max(0) as u64,
                size: info.i64Size.max(0) as u64,
            });
        }
    }
    bins
}

#[tauri::command(async)]
pub fn cleanup_recycle_bins() -> Vec<RecycleBin> {
    query_recycle_bins()
}

#[tauri::command]
pub async fn empty_cleanup_recycle_bin(
    app: AppHandle,
    state: State<'_, CleanupState>,
    drive: String,
    acknowledged: bool,
) -> AppResult<Vec<RecycleBin>> {
    use windows_sys::Win32::UI::Shell::{
        SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW,
    };
    if !acknowledged {
        return Err(AppError::new("cleanupNotAcknowledged"));
    }
    if !query_recycle_bins().iter().any(|bin| bin.drive == drive) {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let (operation, _) = state.begin_kind("cleanupRecycleBin")?;
    operation.attach(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let wide = crate::win::wide(&drive);
        // SAFETY: fixed whitelisted root, explicit user confirmation in the app.
        let result = unsafe {
            SHEmptyRecycleBinW(
                std::ptr::null_mut(),
                wide.as_ptr(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
        };
        if result < 0 {
            return Err(AppError::with_detail(
                "cleanupFailed",
                format!("Windows HRESULT {result:#x}"),
            ));
        }
        _operation.finish(false);
        Ok(query_recycle_bins())
    })
    .await
    .map_err(|error| AppError::with_detail("cleanupFailed", error))?
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "discreveal-cleanup-{}-{}",
                std::process::id(),
                rand::random::<u64>()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn candidate(&self, name: &str) -> Candidate {
            let path = self.0.join(name);
            fs::write(&path, b"discreveal cleanup fixture").unwrap();
            let (identity, allocated) = snapshot(&metadata_file(&path, false).unwrap()).unwrap();
            Candidate {
                id: 1,
                group: "fixture".into(),
                root: self.0.clone(),
                path,
                identity,
                allocated,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn expanded_selection_is_catalog_bound_and_rejects_unknown_and_duplicate_ids() {
        let rules: Vec<_> = (0..30)
            .map(|n| {
                let mut rule = fixture_rule(PathBuf::new());
                rule.id = format!("rule-{n}");
                rule
            })
            .collect();
        let ids: Vec<_> = rules.iter().map(|rule| rule.id.clone()).collect();
        assert!(valid_rule_ids(&ids, &rules));
        assert!(!valid_rule_ids(&["unknown".into()], &rules));
        assert!(!valid_rule_ids(&["rule-1".into(), "rule-1".into()], &rules));
        assert!(!valid_rule_ids(&[], &rules));
    }
    #[test]
    fn every_relevant_process_blocks_a_provider_and_unknown_state_fails_closed() {
        let names = vec!["main.exe".into(), "helper.exe".into()];
        assert!(blocked_processes(
            &names,
            Some(&HashSet::from(["helper.exe".into()]))
        ));
        assert!(blocked_processes(&names, None));
        assert!(!blocked_processes(&names, Some(&HashSet::new())));
        assert!(!blocked_processes(&[], None));
    }
    #[test]
    fn windows_cleanup_arguments_never_request_silent_or_global_deletion() {
        assert_eq!(
            windows_cleanup_args("C:\\"),
            Some(["/d".into(), "C".into()])
        );
        for invalid in [
            "C:",
            "C:\\ /VERYLOWDISK",
            "C:\\folder",
            "c:\\",
            "\\\\server\\share",
            "1:\\",
        ] {
            assert!(windows_cleanup_args(invalid).is_none());
        }
    }
    #[test]
    fn log_provider_preserves_non_log_files_in_the_same_directory() {
        let fixture = Fixture::new();
        let log = fixture.candidate("old.log");
        make_old(&log.path);
        let private = fixture.candidate("recovery.txt");
        make_old(&private.path);
        let mut rule = fixture_rule(fixture.0.clone());
        rule.log_only = true;
        let AnalysisResultAndPlan(_, plan) = analyze(
            vec![rule],
            AnalysisOptions {
                rule_ids: vec!["fixture".into()],
                min_days: 7,
            },
            1,
            &AtomicBool::new(false),
            |_| {},
        );
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, log.path);
        assert!(private.path.exists());
    }
    #[test]
    fn build_registration_requires_the_cache_marker_and_preserves_project_and_outputs() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'",
        )
        .unwrap();
        assert!(cargo_build_rule(&fixture.0).is_err());
        fs::create_dir_all(fixture.0.join("target/debug/deps")).unwrap();
        fs::create_dir_all(fixture.0.join("target/debug/manual-documents")).unwrap();
        fs::write(
            fixture.0.join("target/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n",
        )
        .unwrap();
        let rule = cargo_build_rule(&fixture.0).unwrap();
        assert_eq!(rule.roots, vec![fixture.0.join("target/debug/deps")]);
        assert!(!rule.recommended);
    }
    #[test]
    fn unchanged_file_is_removed_without_touching_other_files() {
        let fixture = Fixture::new();
        let file = fixture.candidate("cache.bin");
        fs::write(fixture.0.join("keep.txt"), b"personal data").unwrap();
        remove_candidate(&file).unwrap();
        assert!(!file.path.exists());
        assert!(fixture.0.join("keep.txt").exists());
    }
    #[test]
    fn changed_file_is_preserved() {
        let fixture = Fixture::new();
        let file = fixture.candidate("cache.bin");
        fs::write(&file.path, b"changed personal content, not the old cache").unwrap();
        assert_eq!(remove_candidate(&file), Err("changed"));
        assert!(file.path.exists());
    }
    #[test]
    fn hardlinked_file_is_preserved() {
        let fixture = Fixture::new();
        let file = fixture.candidate("cache.bin");
        fs::hard_link(&file.path, fixture.0.join("another.bin")).unwrap();
        assert_eq!(remove_candidate(&file), Err("unsafe"));
        assert!(file.path.exists());
    }
    #[test]
    fn locked_file_is_preserved() {
        let fixture = Fixture::new();
        let file = fixture.candidate("cache.bin");
        let _held = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file.path)
            .unwrap();
        assert_eq!(remove_candidate(&file), Err("inUse"));
        assert!(file.path.exists());
    }
    #[test]
    fn outside_root_is_preserved() {
        let fixture = Fixture::new();
        let mut file = fixture.candidate("cache.bin");
        file.root = fixture.0.join("other");
        assert_eq!(remove_candidate(&file), Err("unsafe"));
        assert!(file.path.exists());
    }
    #[test]
    fn cancellation_and_single_operation_are_enforced() {
        let state = CleanupState::default();
        let (operation, cancel) = state.begin().unwrap();
        assert!(state.begin().is_err());
        cancel.store(true, Ordering::Relaxed);
        assert!(cancel.load(Ordering::Relaxed));
        drop(operation);
        assert!(state.begin().is_ok());
    }
    #[test]
    fn recent_and_future_files_do_not_pass_age_filter() {
        let now = 1_800_000_000;
        let stamp = |seconds| (seconds + 11_644_473_600) * 10_000_000;
        assert!(!old_enough(stamp(now), now, 7));
        assert!(!old_enough(stamp(now + DAY), now, 7));
        assert!(old_enough(stamp(now - 7 * DAY), now, 7));
        assert!(!old_enough(0, now, 7));
    }
    fn fixture_rule(root: PathBuf) -> Rule {
        Rule {
            id: "fixture".into(),
            label: "fixture".into(),
            effect: "fixture".into(),
            recommended: false,
            available: true,
            blocked: false,
            min_days: 7,
            category: "windows".into(),
            processes: vec![],
            log_only: false,
            roots: vec![root],
        }
    }
    fn make_old(path: &Path) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(31 * DAY)),
            )
            .unwrap();
    }
    #[test]
    fn a_large_cache_does_not_starve_later_areas_of_the_analysis_budget() {
        let first = Fixture::new();
        let second = Fixture::new();
        for index in 0..4 {
            make_old(&first.candidate(&format!("old-{index}.bin")).path);
        }
        let later = second.candidate("later.bin");
        make_old(&later.path);
        let first_rule = fixture_rule(first.0.clone());
        let mut second_rule = fixture_rule(second.0.clone());
        second_rule.id = "later".into();
        let AnalysisResultAndPlan(result, plan) = analyze_with_budget(
            vec![first_rule, second_rule],
            AnalysisOptions {
                rule_ids: vec!["fixture".into(), "later".into()],
                min_days: 7,
            },
            1,
            &AtomicBool::new(false),
            4,
            |_| {},
        );
        assert_eq!(
            plan.files
                .iter()
                .filter(|item| item.group == "fixture")
                .count(),
            3
        );
        assert_eq!(plan.files.len(), 4, "unused quotas must be redistributed");
        assert!(plan.files.iter().any(|item| item.path == later.path));
        assert_eq!(result.limited_groups, vec!["fixture"]);
        assert!(result.truncated);
        assert!(later.path.exists());
    }
    // Manual read-only yield audit: uses real provider metadata, never executes a plan.
    #[test]
    #[ignore = "manual metadata-only audit of the current Windows user"]
    fn read_only_yield_audit() {
        let current = discover_rules().unwrap();
        let mut former = current.clone();
        for rule in &mut former {
            if (rule.category == "browsers" && rule.id != "firefox") || rule.id == "teams" {
                rule.roots.retain(|root| {
                    let name = root.file_name().unwrap_or_default().to_string_lossy();
                    if rule.id == "teams" && !root.to_string_lossy().contains("EBWebView") {
                        return true;
                    }
                    name == "Cache" || name == "Code Cache"
                });
            }
        }
        let mut reports = Vec::new();
        for (name, rules, days, extra) in [
            ("former-default", former, 7, false),
            ("new-default", current.clone(), 1, false),
            ("new-with-additional-caches", current, 1, true),
        ] {
            let ids = rules
                .iter()
                .filter(|rule| {
                    rule.available
                        && (extra || !["shaders", "development"].contains(&rule.category.as_str()))
                })
                .map(|rule| rule.id.clone())
                .collect();
            let blocked: Vec<_> = rules
                .iter()
                .filter(|rule| rule.available && rule.blocked)
                .map(|rule| rule.id.clone())
                .collect();
            let AnalysisResultAndPlan(result, _) = analyze(
                rules,
                AnalysisOptions {
                    rule_ids: ids,
                    min_days: days,
                },
                1,
                &AtomicBool::new(false),
                |_| {},
            );
            let total: u64 = result.groups.iter().map(|group| group.allocated).sum();
            reports.push(serde_json::json!({"mode": name, "allocatedBytes": total, "blocked": blocked, "result": result}));
        }
        println!(
            "DISCREVEAL_YIELD_AUDIT={}",
            serde_json::to_string(&reports).unwrap()
        );
    }
    #[test]
    fn lower_global_age_respects_each_providers_minimum() {
        let cache = Fixture::new();
        let temp = Fixture::new();
        for fixture in [&cache, &temp] {
            let item = fixture.candidate("two-days.bin");
            File::options()
                .write(true)
                .open(&item.path)
                .unwrap()
                .set_times(
                    fs::FileTimes::new()
                        .set_modified(SystemTime::now() - Duration::from_secs(2 * DAY)),
                )
                .unwrap();
        }
        let mut cache_rule = fixture_rule(cache.0.clone());
        cache_rule.id = "cache".into();
        cache_rule.min_days = 1;
        let temp_rule = fixture_rule(temp.0.clone());
        let AnalysisResultAndPlan(_, plan) = analyze(
            vec![cache_rule, temp_rule],
            AnalysisOptions {
                rule_ids: vec!["cache".into(), "fixture".into()],
                min_days: 1,
            },
            1,
            &AtomicBool::new(false),
            |_| {},
        );
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].group, "cache");
        assert!(temp.0.join("two-days.bin").exists());
    }
    #[test]
    fn analysis_filters_age_readonly_and_hardlinks_and_leaves_files_untouched() {
        let fixture = Fixture::new();
        let old = fixture.candidate("old.bin");
        make_old(&old.path);
        let fresh = fixture.candidate("new.bin");
        let linked = fixture.candidate("linked.bin");
        make_old(&linked.path);
        fs::hard_link(&linked.path, fixture.0.join("another.bin")).unwrap();
        let readonly = fixture.candidate("read-only.bin");
        make_old(&readonly.path);
        let mut permissions = fs::metadata(&readonly.path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&readonly.path, permissions).unwrap();
        let AnalysisResultAndPlan(result, plan) = analyze(
            vec![fixture_rule(fixture.0.clone())],
            AnalysisOptions {
                rule_ids: vec!["fixture".into()],
                min_days: 7,
            },
            8,
            &AtomicBool::new(false),
            |_| {},
        );
        assert_eq!(result.plan_id, 8);
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, old.path);
        assert!(fresh.path.exists());
        assert!(old.path.exists());
        assert!(result.skipped >= 3);
        #[allow(clippy::permissions_set_readonly_false)]
        {
            let mut permissions = fs::metadata(&readonly.path).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&readonly.path, permissions).unwrap();
        }
    }
    #[test]
    fn cancelled_analysis_does_not_walk_or_mutate() {
        let fixture = Fixture::new();
        let file = fixture.candidate("old.bin");
        make_old(&file.path);
        let AnalysisResultAndPlan(result, plan) = analyze(
            vec![fixture_rule(fixture.0.clone())],
            AnalysisOptions {
                rule_ids: vec!["fixture".into()],
                min_days: 7,
            },
            1,
            &AtomicBool::new(true),
            |_| {},
        );
        assert!(result.cancelled);
        assert!(plan.files.is_empty());
        assert!(file.path.exists());
    }
    #[test]
    fn held_parents_cannot_be_swapped() {
        let fixture = Fixture::new();
        let folder = fixture.0.join("cache");
        fs::create_dir(&folder).unwrap();
        let held = hold_ancestors(&folder).unwrap();
        assert!(fs::rename(&folder, fixture.0.join("swapped")).is_err());
        drop(held);
        fs::rename(&folder, fixture.0.join("swapped")).unwrap();
    }
    #[test]
    fn junctions_are_skipped_during_analysis_and_rejected_after_parent_swap() {
        let fixture = Fixture::new();
        let target = fixture.0.join("private");
        fs::create_dir(&target).unwrap();
        let private = target.join("important.txt");
        fs::write(&private, b"keep this personal data").unwrap();
        make_old(&private);
        let cache = fixture.0.join("cache");
        fs::create_dir(&cache).unwrap();
        let original = cache.join("important.txt");
        fs::write(&original, b"old cache").unwrap();
        make_old(&original);
        let (identity, allocated) = snapshot(&metadata_file(&original, false).unwrap()).unwrap();
        let candidate = Candidate {
            id: 1,
            group: "fixture".into(),
            path: original,
            root: fixture.0.clone(),
            identity,
            allocated,
        };
        fs::remove_dir_all(&cache).unwrap();
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&cache)
            .arg(&target)
            .output()
            .unwrap();
        assert!(status.status.success(), "junction fixture creation failed");
        assert_eq!(remove_candidate(&candidate), Err("unsafe"));
        let AnalysisResultAndPlan(_, plan) = analyze(
            vec![fixture_rule(cache.clone())],
            AnalysisOptions {
                rule_ids: vec!["fixture".into()],
                min_days: 7,
            },
            1,
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(plan.files.is_empty());
        assert_eq!(fs::read(&private).unwrap(), b"keep this personal data");
        fs::remove_dir(&cache).unwrap();
    }
}
