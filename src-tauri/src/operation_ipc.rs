//! Tauri adapter for the pure lifecycle contract; this module never touches user files.
use crate::operations::{Operation, Snapshot};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
pub(crate) fn attach(operation: &Operation, app: &AppHandle) {
    let app = app.clone();
    operation.set_sink(Arc::new(move |snapshot| {
        let _ = app.emit("operation:status", snapshot);
    }));
}
#[tauri::command]
pub(crate) fn get_operation_statuses() -> Vec<Snapshot> {
    crate::operations::snapshots()
}
