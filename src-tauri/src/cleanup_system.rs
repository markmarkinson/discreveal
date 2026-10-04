//! discReveal by markMarkinson: Windows-owned cleanup, selected in our UI.
//! Only compiled Microsoft providers; no registry presets or direct system deletion.
use super::*;
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize, IClassFactory,
};
use windows::Win32::System::Registry::HKEY;
use windows::Win32::UI::LegacyWindowsEnvironmentFeatures::*;
use windows::core::{GUID, HRESULT, Interface, PCWSTR, PWSTR, implement};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows_sys::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_SZ, RegCloseKey, RegOpenKeyExW,
    RegQueryValueExW,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const REG_BASE: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VolumeCaches";
struct Provider {
    id: &'static str,
    key: &'static str,
    dll: &'static str,
    clsid: u128,
    admin: bool,
    rollback: bool,
}
// Excludes Downloads, driver packages, ESD/recovery media, restore points, user versions.
const PROVIDERS: &[Provider] = &[
    Provider {
        id: "update",
        key: "Update Cleanup",
        dll: "scavengeui.dll",
        clsid: 0x606b3777_3051_401f_974a_e66aca82a3a3,
        admin: true,
        rollback: true,
    },
    Provider {
        id: "delivery",
        key: "Delivery Optimization Files",
        dll: "domgmt.dll",
        clsid: 0x4057c1ad_a51f_40bb_b960_22888ceb9812,
        admin: true,
        rollback: false,
    },
    Provider {
        id: "setup",
        key: "Temporary Setup Files",
        dll: "setupcln.dll",
        clsid: 0x84e04a55_2d42_4909_86e3_62fd11483e8b,
        admin: true,
        rollback: false,
    },
    Provider {
        id: "upgrade",
        key: "Upgrade Discarded Files",
        dll: "setupcln.dll",
        clsid: 0x84e04a55_2d42_4909_86e3_62fd11483e8b,
        admin: true,
        rollback: false,
    },
    Provider {
        id: "upgrade-logs",
        key: "Windows Upgrade Log Files",
        dll: "setupcln.dll",
        clsid: 0x84e04a55_2d42_4909_86e3_62fd11483e8b,
        admin: true,
        rollback: false,
    },
    Provider {
        id: "previous",
        key: "Previous Installations",
        dll: "setupcln.dll",
        clsid: 0x84e04a55_2d42_4909_86e3_62fd11483e8b,
        admin: true,
        rollback: true,
    },
    Provider {
        id: "thumbnails",
        key: "Thumbnail Cache",
        dll: "thumbcache.dll",
        clsid: 0x889900c3_59f3_4c2f_ae21_a409ea01e605,
        admin: false,
        rollback: false,
    },
];

#[derive(Clone)]
pub(super) struct SystemPlan {
    id: u64,
    drive: String,
    allowed: Vec<String>,
}

fn complete_system_plan(inner: &Mutex<Inner>, mut plan: SystemPlan, completed: &[String]) {
    plan.allowed.retain(|id| !completed.contains(id));
    lock(inner).system_plan = (!plan.allowed.is_empty()).then_some(plan);
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemGroup {
    id: &'static str,
    size: u64,
    status: &'static str,
    detail: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAnalysis {
    plan_id: u64,
    drive: String,
    elevated: bool,
    groups: Vec<SystemGroup>,
    cancelled: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemExecution {
    completed: Vec<String>,
    failed: Vec<SystemGroup>,
    cancelled: bool,
    reported_freed: u64,
    observed_free_change: Option<i64>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemProgress {
    plan_id: u64,
    group_id: &'static str,
    phase: &'static str,
    bytes: u64,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn elevated() -> bool {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut value = TOKEN_ELEVATION::default();
        let mut length = 0;
        let okay = GetTokenInformation(
            token,
            TokenElevation,
            (&mut value as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        );
        CloseHandle(token);
        okay != 0 && value.TokenIsElevated != 0
    }
}
struct Apartment;
impl Apartment {
    fn new() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Key(windows_sys::Win32::System::Registry::HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn open_key(provider: &Provider) -> windows::core::Result<Key> {
    let name = wide(&format!("{REG_BASE}\\{}", provider.key));
    let mut key = std::ptr::null_mut();
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            name.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    if code != 0 {
        return Err(windows::core::Error::from_hresult(HRESULT::from_win32(
            code,
        )));
    }
    let key = Key(key);
    let mut value = [0u16; 80];
    let mut size = std::mem::size_of_val(&value) as u32;
    let mut kind = 0;
    let code = unsafe {
        RegQueryValueExW(
            key.0,
            std::ptr::null(),
            std::ptr::null(),
            &mut kind,
            value.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if code != 0 || kind != REG_SZ || size < 2 || size as usize > std::mem::size_of_val(&value) {
        return Err(windows::core::Error::from_hresult(HRESULT(
            0x8007000Du32 as i32,
        )));
    }
    let actual = String::from_utf16_lossy(&value[..(size as usize / 2).saturating_sub(1)]);
    let expected = format!("{{{:?}}}", GUID::from_u128(provider.clsid));
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(windows::core::Error::from_hresult(HRESULT(
            0x8007000Du32 as i32,
        )));
    }
    Ok(key)
}
struct Handler {
    cache: IEmptyVolumeCache,
    _key: Key,
}
trait Cleaner {
    fn scan(&self, callback: &IEmptyVolumeCacheCallBack) -> windows::core::Result<u64>;
    fn purge(&self, callback: &IEmptyVolumeCacheCallBack) -> windows::core::Result<()>;
}
impl Cleaner for Handler {
    fn scan(&self, cb: &IEmptyVolumeCacheCallBack) -> windows::core::Result<u64> {
        let mut size = 0;
        unsafe {
            self.cache.GetSpaceUsed(&mut size, cb)?;
        }
        Ok(size)
    }
    fn purge(&self, cb: &IEmptyVolumeCacheCallBack) -> windows::core::Result<()> {
        unsafe { self.cache.Purge(u64::MAX, cb) }
    }
}
fn clean_handler(
    handler: &impl Cleaner,
    scan: &IEmptyVolumeCacheCallBack,
    purge: &IEmptyVolumeCacheCallBack,
    cancel: &AtomicBool,
) -> windows::core::Result<()> {
    let size = handler.scan(scan)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(windows::core::Error::from_hresult(HRESULT(
            0x80004004u32 as i32,
        )));
    }
    if size > 0 {
        handler.purge(purge)?;
    }
    Ok(())
}
impl Drop for Handler {
    fn drop(&mut self) {
        unsafe {
            let _ = self.cache.Deactivate();
        }
    }
}
fn handler(provider: &Provider, drive: &str, consent: bool) -> windows::core::Result<Handler> {
    let key = open_key(provider)?;
    let path = catalog::windows_directory(true)
        .map_err(|_| windows::core::Error::from_hresult(HRESULT(0x80004005u32 as i32)))?
        .join(provider.dll);
    // Load exclusively from the actual system directory. Never CoCreateInstance:
    // a per-user CLSID override must not substitute executable code here.
    // Keep each server loaded for process lifetime (COM may retain internal state).
    static MODULES: std::sync::OnceLock<Mutex<HashMap<&'static str, usize>>> =
        std::sync::OnceLock::new();
    let mut modules = lock(MODULES.get_or_init(|| Mutex::new(HashMap::new())));
    let library = if let Some(value) = modules.get(provider.dll) {
        *value as windows_sys::Win32::Foundation::HMODULE
    } else {
        let name = wide(&path.to_string_lossy());
        let library = unsafe {
            LoadLibraryExW(
                name.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if library.is_null() {
            return Err(windows::core::Error::from_win32());
        }
        modules.insert(provider.dll, library as usize);
        library
    };
    drop(modules);
    type GetClass =
        unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut std::ffi::c_void) -> HRESULT;
    let proc = unsafe { GetProcAddress(library, c"DllGetClassObject".as_ptr().cast()) }
        .ok_or_else(windows::core::Error::from_win32)?;
    let get_class: GetClass = unsafe { std::mem::transmute(proc) };
    let mut raw = std::ptr::null_mut();
    unsafe {
        get_class(
            &GUID::from_u128(provider.clsid),
            &IClassFactory::IID,
            &mut raw,
        )
        .ok()?;
    }
    let factory = unsafe { IClassFactory::from_raw(raw) };
    let cache: IEmptyVolumeCache = unsafe { factory.CreateInstance(None)? };
    let volume = wide(drive);
    let key_name = wide(provider.key);
    let (mut name, mut description, mut text) = (PWSTR::null(), PWSTR::null(), PWSTR::null());
    // Never SETTINGSMODE or OUTOFDISKSPACE: Initialize must not clean anything.
    let mut flags = if consent {
        EVCF_USERCONSENTOBTAINED
    } else {
        EMPTY_VOLUME_CACHE_FLAGS(0)
    };
    let initialized = unsafe {
        if let Ok(ex) = cache.cast::<IEmptyVolumeCache2>() {
            ex.InitializeEx(
                HKEY(key.0),
                PCWSTR(volume.as_ptr()),
                PCWSTR(key_name.as_ptr()),
                &mut name,
                &mut description,
                &mut text,
                &mut flags,
            )
        } else {
            cache.Initialize(
                HKEY(key.0),
                PCWSTR(volume.as_ptr()),
                &mut name,
                &mut description,
                &mut flags,
            )
        }
    };
    unsafe {
        for p in [name, description, text] {
            if !p.is_null() {
                CoTaskMemFree(Some(p.0.cast()));
            }
        }
    }
    if let Err(error) = initialized {
        unsafe {
            let _ = cache.Deactivate();
        }
        return Err(error);
    }
    Ok(Handler { cache, _key: key })
}

#[implement(IEmptyVolumeCacheCallBack)]
struct Callback {
    cancel: Arc<AtomicBool>,
    app: Option<AppHandle>,
    id: u64,
    group: &'static str,
    purging: AtomicBool,
    freed: Arc<AtomicU64>,
    last: Mutex<Instant>,
}
impl Callback {
    fn report(&self, bytes: u64, phase: &'static str) -> windows::core::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(windows::core::Error::from_hresult(HRESULT(
                0x80004004u32 as i32,
            )));
        }
        let mut last = lock(&self.last);
        if last.elapsed() >= Duration::from_millis(150) {
            if let Some(app) = &self.app {
                let _ = app.emit(
                    "cleanup:system-progress",
                    SystemProgress {
                        plan_id: self.id,
                        group_id: self.group,
                        phase,
                        bytes,
                    },
                );
            }
            *last = Instant::now();
        }
        Ok(())
    }
}
#[allow(non_snake_case)]
impl IEmptyVolumeCacheCallBack_Impl for Callback_Impl {
    fn ScanProgress(&self, bytes: u64, _: u32, _: &PCWSTR) -> windows::core::Result<()> {
        self.report(bytes, "scan")
    }
    fn PurgeProgress(&self, bytes: u64, _: u64, _: u32, _: &PCWSTR) -> windows::core::Result<()> {
        if self.purging.load(Ordering::Relaxed) {
            self.freed.fetch_max(bytes, Ordering::Relaxed);
        }
        self.report(bytes, "purge")
    }
}
fn callback(
    app: Option<AppHandle>,
    id: u64,
    group: &'static str,
    cancel: Arc<AtomicBool>,
    purging: bool,
    freed: Arc<AtomicU64>,
) -> IEmptyVolumeCacheCallBack {
    Callback {
        cancel,
        app,
        id,
        group,
        purging: AtomicBool::new(purging),
        freed,
        last: Mutex::new(Instant::now() - Duration::from_secs(1)),
    }
    .into()
}
fn status(error: &windows::core::Error) -> &'static str {
    match error.code().0 as u32 {
        0x80070005 => "admin",
        0x80070002 | 0x80070003 | 0x80040154 => "unavailable",
        0x80004004 => "cancelled",
        _ => "failed",
    }
}
fn analyze(
    drive: String,
    id: u64,
    cancel: Arc<AtomicBool>,
    app: Option<AppHandle>,
) -> AppResult<(SystemAnalysis, SystemPlan)> {
    let _apartment = Apartment::new().map_err(|e| AppError::with_detail("cleanupFailed", e))?;
    let elevated = elevated();
    let mut groups = Vec::new();
    let mut allowed = Vec::new();
    for provider in PROVIDERS {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if let Some(app) = &app {
            let _ = app.emit(
                "cleanup:system-progress",
                SystemProgress {
                    plan_id: id,
                    group_id: provider.id,
                    phase: "scan",
                    bytes: 0,
                },
            );
        }
        let mut group = SystemGroup {
            id: provider.id,
            size: 0,
            status: "ready",
            detail: None,
        };
        let result = if provider.admin && !elevated {
            open_key(provider).map(|_| {
                group.status = "admin";
            })
        } else {
            handler(provider, &drive, false).and_then(|handler| unsafe {
                handler.cache.GetSpaceUsed(
                    &mut group.size,
                    &callback(
                        app.clone(),
                        id,
                        provider.id,
                        cancel.clone(),
                        false,
                        Arc::new(AtomicU64::new(0)),
                    ),
                )
            })
        };
        if let Err(error) = result {
            group.status = status(&error);
            group.size = 0;
            group.detail = Some(format!("0x{:08X}", error.code().0 as u32));
        }
        if group.status == "ready" && group.size > 0 && !cancel.load(Ordering::Relaxed) {
            allowed.push(provider.id.into());
        }
        groups.push(group);
    }
    let plan = SystemPlan {
        id,
        drive: drive.clone(),
        allowed,
    };
    Ok((
        SystemAnalysis {
            plan_id: id,
            drive,
            elevated,
            groups,
            cancelled: cancel.load(Ordering::Relaxed),
        },
        plan,
    ))
}
fn valid_selection(plan: &SystemPlan, ids: &[String], rollback: bool) -> bool {
    !ids.is_empty()
        && ids.len() <= PROVIDERS.len()
        && ids.iter().collect::<HashSet<_>>().len() == ids.len()
        && ids.iter().all(|id| {
            plan.allowed.contains(id)
                && PROVIDERS
                    .iter()
                    .any(|p| p.id == id && (!p.rollback || rollback))
        })
}
fn valid_drive(drive: &str) -> bool {
    windows_cleanup_args(drive).is_some() && crate::drives::is_fixed_drive(Path::new(drive))
}

#[tauri::command]
pub async fn analyze_windows_cleanup(
    app: AppHandle,
    state: State<'_, CleanupState>,
    drive: Option<String>,
) -> AppResult<SystemAnalysis> {
    let drive = match drive {
        Some(drive) => drive,
        None => catalog::windows_directory(false)?
            .ancestors()
            .last()
            .ok_or_else(|| AppError::new("cleanupInvalidOptions"))?
            .to_string_lossy()
            .into_owned(),
    };
    if !valid_drive(&drive) {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let (operation, cancel) = state.begin_kind("windowsAnalyze")?;
    operation.attach(&app);
    let id = state.sequence.fetch_add(1, Ordering::Relaxed) + 1;
    let inner = state.inner.clone();
    lock(&inner).system_plan = None;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let (result, plan) = analyze(drive, id, cancel, Some(app))?;
        lock(&inner).system_plan = Some(plan);
        _operation.finish(
            result.cancelled
                || result
                    .groups
                    .iter()
                    .any(|group| matches!(group.status, "failed" | "unavailable")),
        );
        Ok(result)
    })
    .await
    .map_err(|e| AppError::with_detail("cleanupFailed", e))?
}

#[tauri::command]
pub async fn execute_windows_cleanup(
    app: AppHandle,
    state: State<'_, CleanupState>,
    plan_id: u64,
    group_ids: Vec<String>,
    acknowledged: bool,
    rollback_acknowledged: bool,
) -> AppResult<SystemExecution> {
    if !acknowledged {
        return Err(AppError::new("cleanupNotAcknowledged"));
    }
    let (operation, cancel) = state.begin_kind("windowsExecute")?;
    operation.attach(&app);
    let plan = {
        let mut inner = lock(&state.inner);
        let plan = inner
            .system_plan
            .as_ref()
            .filter(|p| p.id == plan_id)
            .cloned()
            .ok_or_else(|| AppError::new("cleanupStalePlan"))?;
        if !valid_selection(&plan, &group_ids, rollback_acknowledged) || !valid_drive(&plan.drive) {
            return Err(AppError::new("cleanupInvalidOptions"));
        }
        if !elevated()
            && group_ids
                .iter()
                .any(|id| PROVIDERS.iter().any(|p| p.id == id && p.admin))
        {
            return Err(AppError::new("cleanupAdminRequired"));
        }
        inner.system_plan = None;
        // Preserve the independent cache plan. Every cache candidate is identity-checked.
        plan
    };
    let inner = state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let _apartment = Apartment::new().map_err(|e| AppError::with_detail("cleanupFailed", e))?;
        let before = free_space(Path::new(&plan.drive));
        let mut result = SystemExecution {
            completed: vec![],
            failed: vec![],
            cancelled: false,
            reported_freed: 0,
            observed_free_change: None,
        };
        for id in group_ids {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let provider = PROVIDERS
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| AppError::new("cleanupStalePlan"))?;
            let freed = Arc::new(AtomicU64::new(0));
            let _ = app.emit(
                "cleanup:system-progress",
                SystemProgress {
                    plan_id: plan.id,
                    group_id: provider.id,
                    phase: "scan",
                    bytes: 0,
                },
            );
            let outcome = handler(provider, &plan.drive, true).and_then(|handler| {
                // Recheck with Windows immediately before the selected category is cleaned.
                clean_handler(
                    &handler,
                    &callback(
                        Some(app.clone()),
                        plan.id,
                        provider.id,
                        cancel.clone(),
                        false,
                        freed.clone(),
                    ),
                    &callback(
                        Some(app.clone()),
                        plan.id,
                        provider.id,
                        cancel.clone(),
                        true,
                        freed.clone(),
                    ),
                    &cancel,
                )
            });
            result.reported_freed = result
                .reported_freed
                .saturating_add(freed.load(Ordering::Relaxed));
            match outcome {
                Ok(()) => result.completed.push(id),
                Err(error) => result.failed.push(SystemGroup {
                    id: provider.id,
                    size: 0,
                    status: status(&error),
                    detail: Some(format!("0x{:08X}", error.code().0 as u32)),
                }),
            }
        }
        result.cancelled = cancel.load(Ordering::Relaxed);
        result.observed_free_change = before
            .zip(free_space(Path::new(&plan.drive)))
            .map(|(a, b)| (b as i128 - a as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64);
        complete_system_plan(&inner, plan, &result.completed);
        _operation.finish(result.cancelled || !result.failed.is_empty());
        Ok(result)
    })
    .await
    .map_err(|e| AppError::with_detail("cleanupFailed", e))?
}

/// UAC is shown only after an explicit UI confirmation. Relaunches this exact
/// executable without any cleanup/command arguments. No elevated helper files.
#[tauri::command]
pub async fn restart_cleanup_as_admin(
    app: AppHandle,
    state: State<'_, CleanupState>,
    acknowledged: bool,
) -> AppResult<()> {
    if !acknowledged {
        return Err(AppError::new("cleanupNotAcknowledged"));
    }
    let (operation, _) = state.begin_kind("cleanupElevation")?;
    operation.attach(&app);
    if elevated() {
        return Err(AppError::new("cleanupInvalidOptions"));
    }
    let executable = wide(&std::env::current_exe()?.to_string_lossy());
    let arguments = wide(&format!("--cleanup-restart-parent={}", std::process::id()));
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut info: windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW =
            unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        let verb = wide("runas");
        info.lpVerb = verb.as_ptr();
        info.lpFile = executable.as_ptr();
        info.lpParameters = arguments.as_ptr();
        info.nShow = windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        if unsafe { windows_sys::Win32::UI::Shell::ShellExecuteExW(&mut info) } == 0 {
            return Err(AppError::with_detail(
                "cleanupElevationFailed",
                std::io::Error::last_os_error(),
            ));
        }
        _operation.finish(false);
        Ok(())
    })
    .await
    .map_err(|e| AppError::with_detail("cleanupFailed", e))?;
    if result.is_ok() {
        app.exit(0);
    }
    result
}

/// Wait for the previous app to release the same WebView2 profile. No file or
/// cleanup parameters are accepted; a foreign process is never terminated.
pub(crate) fn wait_restart_parent() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        return;
    }
    let Some(pid) = args[1]
        .strip_prefix("--cleanup-restart-parent=")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|p| *p != 0 && *p != std::process::id())
    else {
        return;
    };
    unsafe {
        use windows_sys::Win32::System::Threading::*;
        let parent = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
        if parent.is_null() {
            return;
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        if QueryFullProcessImageNameW(parent, 0, buffer.as_mut_ptr(), &mut length) != 0 {
            let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
            if std::env::current_exe().is_ok_and(|exe| exe == path) {
                let _ = WaitForSingleObject(parent, 15_000);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        CloseHandle(parent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_and_system_completions_preserve_the_other_part_of_the_batch() {
        let state = CleanupState::default();
        let system = SystemPlan {
            id: 8,
            drive: "C:\\".into(),
            allowed: vec!["thumbnails".into(), "update".into()],
        };
        lock(&state.inner).system_plan = Some(system.clone());
        complete_cache_plan(
            &state.inner,
            Plan {
                id: 7,
                rules: vec![],
                files: vec![],
            },
        );
        assert_eq!(lock(&state.inner).system_plan.as_ref().unwrap().id, 8);
        complete_system_plan(&state.inner, system, &["thumbnails".into()]);
        let inner = lock(&state.inner);
        assert_eq!(inner.plan.as_ref().unwrap().id, 7);
        assert_eq!(inner.system_plan.as_ref().unwrap().allowed, vec!["update"]);
    }
    #[test]
    fn system_purge_requires_a_successful_nonempty_fresh_scan_and_obeys_cancel() {
        struct Fake {
            size: u64,
            fail: bool,
            stop: bool,
            calls: std::cell::Cell<usize>,
            cancel: Arc<AtomicBool>,
        }
        impl Cleaner for Fake {
            fn scan(&self, _: &IEmptyVolumeCacheCallBack) -> windows::core::Result<u64> {
                if self.fail {
                    return Err(windows::core::Error::from_hresult(HRESULT(
                        0x80070005u32 as i32,
                    )));
                }
                if self.stop {
                    self.cancel.store(true, Ordering::Relaxed);
                }
                Ok(self.size)
            }
            fn purge(&self, _: &IEmptyVolumeCacheCallBack) -> windows::core::Result<()> {
                self.calls.set(self.calls.get() + 1);
                Ok(())
            }
        }
        for (size, fail, stop, expected) in [
            (100, false, false, 1),
            (0, false, false, 0),
            (100, true, false, 0),
            (100, false, true, 0),
        ] {
            let cancel = Arc::new(AtomicBool::new(false));
            let handler = Fake {
                size,
                fail,
                stop,
                calls: std::cell::Cell::new(0),
                cancel: cancel.clone(),
            };
            let cb = callback(
                None,
                1,
                "thumbnails",
                cancel.clone(),
                false,
                Arc::new(AtomicU64::new(0)),
            );
            let result = clean_handler(&handler, &cb, &cb, &cancel);
            assert_eq!(handler.calls.get(), expected);
            assert_eq!(result.is_err(), fail || stop);
        }
    }
    #[test]
    fn selection_requires_registered_preview_and_separate_rollback_acknowledgement() {
        let plan = SystemPlan {
            id: 1,
            drive: "C:\\".into(),
            allowed: vec!["update".into(), "thumbnails".into()],
        };
        assert!(valid_selection(&plan, &["thumbnails".into()], false));
        assert!(!valid_selection(&plan, &["update".into()], false));
        assert!(valid_selection(&plan, &["update".into()], true));
        for ids in [
            vec![],
            vec!["DownloadsFolder".into()],
            vec!["previous".into()],
            vec!["thumbnails".into(), "thumbnails".into()],
        ] {
            assert!(!valid_selection(&plan, &ids, true));
        }
        // Execution performs a fresh Windows scan before Purge; an arbitrary UI
        // timeout must not invalidate the other half of a confirmed batch.
        assert!(valid_selection(&plan, &["thumbnails".into()], true));
    }
    #[test]
    fn callback_aborts_both_phases_and_counts_only_purge_notifications() {
        let cancel = Arc::new(AtomicBool::new(false));
        let freed = Arc::new(AtomicU64::new(0));
        let cb = callback(None, 1, "thumbnails", cancel.clone(), true, freed.clone());
        unsafe {
            cb.ScanProgress(900, 0, PCWSTR::null()).unwrap();
            cb.PurgeProgress(100, 200, 0, PCWSTR::null()).unwrap();
            cb.PurgeProgress(50, 200, 0, PCWSTR::null()).unwrap();
        }
        assert_eq!(freed.load(Ordering::Relaxed), 100);
        cancel.store(true, Ordering::Relaxed);
        unsafe {
            assert!(cb.ScanProgress(0, 0, PCWSTR::null()).is_err());
            assert!(cb.PurgeProgress(0, 0, 0, PCWSTR::null()).is_err());
        }
    }
    #[test]
    #[ignore = "manual read-only Windows provider preview; never invokes Purge"]
    fn read_only_windows_preview() {
        let _apartment = Apartment::new().unwrap();
        for provider in PROVIDERS {
            match open_key(provider) {
                Ok(_) => println!("{}: registered", provider.id),
                Err(e) => println!("{}: {:?}", provider.id, e),
            }
        }
        let (result, _) =
            analyze("C:\\".into(), 1, Arc::new(AtomicBool::new(false)), None).unwrap();
        println!(
            "DISCREVEAL_SYSTEM_PREVIEW={}",
            serde_json::to_string(&result).unwrap()
        );
        assert!(!result.cancelled);
    }
}
