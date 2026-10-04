//! Restore-compatible Windows recycling with verified completion.
//!
//! Shell deletion is path-based, not handle-bound. PreDeleteItem narrows the
//! rename window; PostDeleteItem checks the actual new Recycle Bin object and
//! refuses to report success for a different identity or permanent deletion.
//! This detects a final replacement; it does not prevent a same-user attacker
//! from replacing the name after the last callback and before the Shell opens it.

use super::{ensure_recycle_identity, file_identity};
use crate::error::{AppError, AppResult};
use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOF_NO_CONNECTED_ELEMENTS, FOF_NO_UI, FOF_RENAMEONCOLLISION, FOFX_EARLYFAILURE,
    FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IFileOperationProgressSink,
    IFileOperationProgressSink_Impl, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, PCWSTR, Ref, implement};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

type Identity = (u64, [u8; 16]);

#[cfg(test)]
type BeforeDelete = Box<dyn FnOnce() -> AppResult<()>>;
#[cfg(test)]
thread_local! {
    static BEFORE_DELETE_HOOK: std::cell::RefCell<Option<BeforeDelete>> = const { std::cell::RefCell::new(None) };
    static BEFORE_ROLLBACK_HOOK: std::cell::RefCell<Option<BeforeDelete>> = const { std::cell::RefCell::new(None) };
    static BEFORE_MOVE_HOOK: std::cell::RefCell<Option<BeforeDelete>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn recycle_checked_with_rollback_hook(
    target: &Path,
    expected: Identity,
    before_delete: BeforeDelete,
    before_rollback: BeforeDelete,
) -> AppResult<()> {
    BEFORE_ROLLBACK_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(before_rollback);
    });
    let result = recycle_checked_with_hook(target, expected, before_delete);
    BEFORE_ROLLBACK_HOOK.with(|slot| {
        slot.borrow_mut().take();
    });
    result
}

#[cfg(test)]
pub(super) fn recycle_checked_with_late_collision(
    target: &Path,
    expected: Identity,
    before_delete: BeforeDelete,
    before_move: BeforeDelete,
) -> AppResult<()> {
    BEFORE_MOVE_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(before_move);
    });
    let result = recycle_checked_with_hook(target, expected, before_delete);
    BEFORE_MOVE_HOOK.with(|slot| {
        slot.borrow_mut().take();
    });
    result
}

#[cfg(test)]
pub(super) fn recycle_checked_with_hook(
    target: &Path,
    expected: Identity,
    hook: BeforeDelete,
) -> AppResult<()> {
    BEFORE_DELETE_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(hook);
    });
    let result = recycle_checked(target, expected);
    BEFORE_DELETE_HOOK.with(|slot| {
        slot.borrow_mut().take();
    });
    result
}

#[derive(Default)]
struct Outcome {
    verified: bool,
    error: Option<AppError>,
    rollback: Option<Rollback>,
    completed_path: Option<PathBuf>,
}

struct Rollback {
    item: IShellItem,
    identity: Identity,
}

#[implement(IFileOperationProgressSink)]
struct RecycleSink {
    target: PathBuf,
    expected: Identity,
    outcome: Rc<RefCell<Outcome>>,
}

impl RecycleSink_Impl {
    fn reject(&self, error: AppError) -> windows::core::Result<()> {
        let mut outcome = self.outcome.borrow_mut();
        // A later Shell abort callback must not obscure the original identity
        // failure that caused it.
        if outcome.error.is_none() {
            outcome.error = Some(error);
        }
        Err(windows::core::Error::from_hresult(HRESULT(
            0x80004004u32 as i32,
        )))
    }
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for RecycleSink_Impl {
    fn PreDeleteItem(&self, _: u32, _: Ref<'_, IShellItem>) -> windows::core::Result<()> {
        match ensure_recycle_identity(&self.target, self.expected) {
            Ok(_) => {}
            Err(error) => return self.reject(error),
        }
        // Test-only adversary at the precise remaining Shell-open boundary;
        // absent from production binaries and never touches user paths.
        #[cfg(test)]
        if let Some(hook) = BEFORE_DELETE_HOOK.with(|slot| slot.borrow_mut().take())
            && let Err(error) = hook()
        {
            return self.reject(error);
        }
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        result: HRESULT,
        created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        if let Err(error) = result.ok() {
            return self.reject(AppError::with_detail("recycleBinFailed", error));
        }
        let Some(created) = created.as_ref() else {
            return self.reject(AppError::new("recycleBinFailed"));
        };
        match recycled_identity(created) {
            Ok(actual) if actual == self.expected => {
                self.outcome.borrow_mut().verified = true;
                Ok(())
            }
            Ok(actual) => {
                self.outcome.borrow_mut().rollback = Some(Rollback {
                    item: created.clone(),
                    identity: actual,
                });
                self.reject(AppError::new("recycleTargetChanged"))
            }
            Err(error) => self.reject(error),
        }
    }

    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _: HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _: u32,
        item: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        match item.as_ref().map(recycled_identity) {
            Some(Ok(actual)) if actual == self.expected => {}
            _ => return self.reject(AppError::new("recycleTargetChanged")),
        }
        #[cfg(test)]
        if let Some(hook) = BEFORE_MOVE_HOOK.with(|slot| slot.borrow_mut().take())
            && let Err(error) = hook()
        {
            return self.reject(error);
        }
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        result: HRESULT,
        created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        if let Err(error) = result.ok() {
            return self.reject(AppError::with_detail("recycleTargetChanged", error));
        }
        let Some(created) = created.as_ref() else {
            return self.reject(AppError::new("recycleTargetChanged"));
        };
        match recycled_identity(created) {
            Ok(actual) if actual == self.expected => match shell_file_path(created) {
                Ok(path) => {
                    let mut outcome = self.outcome.borrow_mut();
                    outcome.verified = true;
                    outcome.completed_path = Some(path);
                    Ok(())
                }
                Err(error) => self.reject(error),
            },
            _ => self.reject(AppError::new("recycleTargetChanged")),
        }
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Ref<'_, IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _: u32, _: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

fn shell_file_path(item: &IShellItem) -> AppResult<PathBuf> {
    // SAFETY: live Shell item belongs to the current STA. This is the new item
    // supplied by Windows, not GetFinalPathName on a stale pre-rename handle.
    let raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }
        .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
    // SAFETY: GetDisplayName returned a NUL-terminated allocation; copy it
    // before releasing it using the documented COM allocator.
    Ok(unsafe {
        let path = PathBuf::from(OsString::from_wide(raw.as_wide()));
        CoTaskMemFree(Some(raw.0.cast()));
        path
    })
}

fn recycled_identity(item: &IShellItem) -> AppResult<Identity> {
    let path = shell_file_path(item)?;
    let file = fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    file_identity(&file)
}

/// Call only inside an owned fresh thread; no COM interfaces cross its boundary.
pub(super) fn recycle_checked(target: &Path, expected: Identity) -> AppResult<()> {
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: this guard follows successful initialization on this thread.
            unsafe { CoUninitialize() };
        }
    }
    // SAFETY: caller uses a dedicated fresh thread with no existing apartment.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
    let _apartment = Apartment;
    let _held = ensure_recycle_identity(target, expected)?;
    let outcome = Rc::new(RefCell::new(Outcome::default()));
    let sink: IFileOperationProgressSink = RecycleSink {
        target: target.to_path_buf(),
        expected,
        outcome: Rc::clone(&outcome),
    }
    .into();
    let name: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: all interfaces and pointers belong to this initialized STA.
    let operation: IFileOperation = unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
        operation
            .SetOperationFlags(
                FOF_NO_UI | FOF_NO_CONNECTED_ELEMENTS | FOFX_RECYCLEONDELETE | FOFX_EARLYFAILURE,
            )
            .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None)
            .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
        operation
            .DeleteItem(&item, &sink)
            .map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
        operation
    };
    let _current = ensure_recycle_identity(target, expected)?;
    // SAFETY: live interface is used in the apartment that created it.
    let performed = unsafe { operation.PerformOperations() };
    let (error, rollback, verified) = {
        let mut result = outcome.borrow_mut();
        (result.error.take(), result.rollback.take(), result.verified)
    };
    if let Some(error) = error {
        if let Some(rollback) = rollback
            && let Err(rollback_error) = restore_unexpected_item(&rollback, target)
        {
            if rollback_error.code == "recycleRecoveryMoved" {
                return Err(rollback_error);
            }
            return Err(AppError::with_detail(
                "recycleTargetChanged",
                rollback_error,
            ));
        }
        return Err(error);
    }
    performed.map_err(|error| AppError::with_detail("recycleBinFailed", error))?;
    // SAFETY: same live STA interface. A successful HRESULT alone is insufficient.
    if unsafe { operation.GetAnyOperationsAborted() }
        .map_err(|error| AppError::with_detail("recycleBinFailed", error))?
        .as_bool()
        || !verified
    {
        return Err(AppError::new("recycleBinFailed"));
    }
    Ok(())
}

/// Undo only the exact newly created unexpected object. The native transfer
/// Shell MoveItem honors RENAMEONCOLLISION, so a late collision cannot overwrite
/// a newly created file. A renamed recovery is reported as partial, with its
/// actual location. Initial collisions leave the object in the bin unchanged.
fn restore_unexpected_item(rollback: &Rollback, target: &Path) -> AppResult<()> {
    #[cfg(test)]
    if let Some(hook) = BEFORE_ROLLBACK_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook()?;
    }
    if recycled_identity(&rollback.item)? != rollback.identity {
        return Err(AppError::new("recycleTargetChanged"));
    }
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(AppError::new("recycleTargetChanged")),
    }
    let parent = target
        .parent()
        .ok_or_else(|| AppError::new("invalidPath"))?;
    let name = target
        .file_name()
        .ok_or_else(|| AppError::new("invalidPath"))?;
    let parent_name: Vec<u16> = parent.as_os_str().encode_wide().chain(Some(0)).collect();
    let leaf_name: Vec<u16> = name.encode_wide().chain(Some(0)).collect();
    let outcome = Rc::new(RefCell::new(Outcome::default()));
    let sink: IFileOperationProgressSink = RecycleSink {
        target: target.to_path_buf(),
        expected: rollback.identity,
        outcome: Rc::clone(&outcome),
    }
    .into();
    // SAFETY: all objects belong to this STA; strings are NUL-terminated and
    // live for the operation. RENAMEONCOLLISION never confirms an overwrite.
    let operation: IFileOperation = unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?;
        operation
            .SetOperationFlags(
                FOF_NO_UI | FOF_NO_CONNECTED_ELEMENTS | FOF_RENAMEONCOLLISION | FOFX_EARLYFAILURE,
            )
            .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?;
        let parent: IShellItem = SHCreateItemFromParsingName(PCWSTR(parent_name.as_ptr()), None)
            .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?;
        operation
            .MoveItem(&rollback.item, &parent, PCWSTR(leaf_name.as_ptr()), &sink)
            .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?;
        operation
    };
    // SAFETY: operation and its callback are owned by this STA.
    unsafe { operation.PerformOperations() }
        .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?;
    let mut outcome = outcome.borrow_mut();
    if let Some(error) = outcome.error.take() {
        return Err(error);
    }
    if !outcome.verified
        || unsafe { operation.GetAnyOperationsAborted() }
            .map_err(|error| AppError::with_detail("recycleTargetChanged", error))?
            .as_bool()
    {
        return Err(AppError::new("recycleTargetChanged"));
    }
    let actual_path = outcome
        .completed_path
        .as_ref()
        .ok_or_else(|| AppError::new("recycleTargetChanged"))?;
    if crate::guard::plain_path(actual_path.clone())
        != crate::guard::plain_path(target.to_path_buf())
    {
        return Err(AppError::with_detail(
            "recycleRecoveryMoved",
            actual_path.display(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_or_missing_created_object_cannot_count_as_recycled() {
        let outcome = Rc::new(RefCell::new(Outcome::default()));
        let sink: IFileOperationProgressSink = RecycleSink {
            target: PathBuf::from("unused-test-path"),
            expected: (0, [0; 16]),
            outcome: Rc::clone(&outcome),
        }
        .into();
        // SAFETY: our local implementation is called directly; no Shell operation
        // or filesystem mutation occurs. NULL is documented for permanent delete.
        assert!(unsafe { sink.PostDeleteItem(0, None, HRESULT(0), None) }.is_err());
        let result = outcome.borrow();
        assert!(!result.verified);
        assert_eq!(result.error.as_ref().unwrap().code, "recycleBinFailed");
    }

    #[test]
    fn actual_delete_failure_cannot_count_as_recycled() {
        let outcome = Rc::new(RefCell::new(Outcome::default()));
        let sink: IFileOperationProgressSink = RecycleSink {
            target: PathBuf::from("unused-test-path"),
            expected: (0, [0; 16]),
            outcome: Rc::clone(&outcome),
        }
        .into();
        // SAFETY: direct call to our sink only; HRESULT is a simulated failure.
        assert!(
            unsafe { sink.PostDeleteItem(0, None, HRESULT(0x80070005u32 as i32), None) }.is_err()
        );
        let result = outcome.borrow();
        assert!(!result.verified);
        assert_eq!(result.error.as_ref().unwrap().code, "recycleBinFailed");
    }
}
