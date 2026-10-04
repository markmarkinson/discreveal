//! Shared lifecycle contract; filesystem algorithms depend on cancellation, not Tauri.
//! Terminal status describes work, never promises rollback. A process abort cannot run guards.
use serde::Serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Weak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

pub(crate) type OperationId = u64;
pub(crate) type CancellationToken = Arc<AtomicBool>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TerminalStatus {
    Running,
    CancelRequested,
    Complete,
    Partial,
    Failed,
    Abandoned,
}
impl TerminalStatus {
    fn terminal(self) -> bool {
        !matches!(self, Self::Running | Self::CancelRequested)
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub operation_id: OperationId,
    pub request_id: Option<u64>,
    pub kind: &'static str,
    pub status: TerminalStatus,
}
struct Inner {
    snapshot: Snapshot,
    sink: Option<Arc<dyn Fn(Snapshot) + Send + Sync>>,
}
#[derive(Clone)]
pub(crate) struct Operation {
    inner: Arc<Mutex<Inner>>,
    cancel: CancellationToken,
    activity: Arc<Activity>,
}
struct Activity {
    started: Instant,
    last_ms: AtomicU64,
}
impl Activity {
    fn touch(&self) {
        self.last_ms
            .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    fn idle_for(&self) -> Duration {
        self.started
            .elapsed()
            .saturating_sub(Duration::from_millis(self.last_ms.load(Ordering::Relaxed)))
    }
}
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
static HISTORY: OnceLock<Mutex<VecDeque<Operation>>> = OnceLock::new();
fn history() -> &'static Mutex<VecDeque<Operation>> {
    HISTORY.get_or_init(Mutex::default)
}
fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}
fn reserve_history(items: &mut VecDeque<Operation>) -> crate::error::AppResult<()> {
    while items.len() >= 64 {
        let Some(index) = items
            .iter()
            .position(|item| lock(&item.inner).snapshot.status.terminal())
        else {
            return Err(crate::error::AppError::new("operationLimit"));
        };
        items.remove(index);
    }
    Ok(())
}
impl Operation {
    pub fn start(
        kind: &'static str,
        request_id: Option<u64>,
        cancel: Arc<AtomicBool>,
    ) -> crate::error::AppResult<Self> {
        let operation = Self {
            inner: Arc::new(Mutex::new(Inner {
                snapshot: Snapshot {
                    operation_id: SEQUENCE.fetch_add(1, Ordering::Relaxed),
                    request_id,
                    kind,
                    status: TerminalStatus::Running,
                },
                sink: None,
            })),
            cancel,
            activity: Arc::new(Activity {
                started: Instant::now(),
                last_ms: AtomicU64::new(0),
            }),
        };
        let mut items = lock(history());
        reserve_history(&mut items)?;
        items.push_back(operation.clone());
        Ok(operation)
    }
    pub fn touch(&self) {
        self.activity.touch();
    }
    pub fn start_readonly_watchdog(&self, timeout: Duration) -> std::io::Result<()> {
        if !matches!(self.snapshot().kind, "scan" | "scanExpand" | "duplicates") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Watchdogs cannot abandon mutation operations",
            ));
        }
        let operation = self.clone();
        std::thread::Builder::new()
            .name("operation-watchdog".into())
            .spawn(move || {
                let interval = timeout
                    .min(Duration::from_millis(200))
                    .max(Duration::from_millis(1));
                loop {
                    std::thread::sleep(interval);
                    if operation.snapshot().status.terminal() {
                        return;
                    }
                    if operation.activity.idle_for() >= timeout {
                        operation.abandon();
                        cancellation_requested(&operation.cancel);
                        return;
                    }
                }
            })?;
        Ok(())
    }
    pub fn set_sink(&self, sink: Arc<dyn Fn(Snapshot) + Send + Sync>) {
        let snapshot = {
            let mut inner = lock(&self.inner);
            inner.sink = Some(sink.clone());
            inner.snapshot.clone()
        };
        sink(snapshot);
    }
    fn transition(&self, status: TerminalStatus) {
        let delivery = {
            let mut inner = lock(&self.inner);
            if inner.snapshot.status.terminal() || inner.snapshot.status == status {
                return;
            }
            inner.snapshot.status = status;
            inner
                .sink
                .as_ref()
                .map(|sink| (sink.clone(), inner.snapshot.clone()))
        };
        if let Some((sink, snapshot)) = delivery {
            sink(snapshot);
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        if self.cancel.load(Ordering::Relaxed) {
            self.transition(TerminalStatus::CancelRequested);
        }
        lock(&self.inner).snapshot.clone()
    }
    pub fn finish(&self, incomplete: bool) {
        self.transition(if incomplete || self.cancel.load(Ordering::Relaxed) {
            TerminalStatus::Partial
        } else {
            TerminalStatus::Complete
        });
    }
    pub fn fail(&self) {
        self.transition(TerminalStatus::Failed);
    }
    pub fn abandon(&self) {
        self.transition(TerminalStatus::Abandoned);
    }
    pub fn guard(&self) -> OperationGuard {
        OperationGuard(Some(self.clone()))
    }
}
pub(crate) struct OperationGuard(Option<Operation>);
impl OperationGuard {
    /// Transfer responsibility to the worker; does not announce a result.
    pub fn handoff(mut self) {
        self.0.take();
    }
}
impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Some(operation) = &self.0 {
            if std::thread::panicking() {
                operation.abandon();
            } else {
                operation.fail();
            }
        }
    }
}
type ActivityCache = (usize, Weak<AtomicBool>, Weak<Activity>);
thread_local! {static ACTIVITY_CACHE: RefCell<Option<ActivityCache>> = const { RefCell::new(None) };}
/// Registered read workers resolve once per token/thread. Each later chunk updates only one atomic counter.
pub(crate) fn touch_token(token: &AtomicBool) {
    let pointer = std::ptr::from_ref(token) as usize;
    ACTIVITY_CACHE.with(|slot| {
        let mut cached = slot.borrow_mut();
        if let Some((saved, weak_token, weak_activity)) = cached.as_ref()
            && *saved == pointer
            && weak_token.upgrade().is_some()
            && let Some(activity) = weak_activity.upgrade()
        {
            activity.touch();
            return;
        }
        let operation = lock(history())
            .iter()
            .find(|item| std::ptr::eq(item.cancel.as_ref(), token))
            .cloned();
        if let Some(operation) = operation {
            operation.touch();
            *cached = Some((
                pointer,
                Arc::downgrade(&operation.cancel),
                Arc::downgrade(&operation.activity),
            ));
        }
    });
}
pub(crate) fn cancellation_requested(token: &Arc<AtomicBool>) {
    token.store(true, Ordering::Relaxed);
    let matching: Vec<_> = lock(history())
        .iter()
        .filter(|operation| Arc::ptr_eq(&operation.cancel, token))
        .cloned()
        .collect();
    for operation in matching {
        operation.transition(TerminalStatus::CancelRequested);
    }
}
pub(crate) fn snapshots() -> Vec<Snapshot> {
    let operations: Vec<_> = lock(history()).iter().cloned().collect();
    operations.iter().map(Operation::snapshot).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn start() -> Operation {
        Operation::start("test", Some(42), Arc::new(AtomicBool::new(false))).unwrap()
    }
    #[test]
    fn cancellation_is_not_terminal_and_completion_cannot_be_rewritten() {
        let op = start();
        assert_eq!(op.snapshot().status, TerminalStatus::Running);
        cancellation_requested(&op.cancel);
        assert_eq!(op.snapshot().status, TerminalStatus::CancelRequested);
        op.finish(false);
        assert_eq!(op.snapshot().status, TerminalStatus::Partial);
        op.fail();
        op.abandon();
        assert_eq!(op.snapshot().status, TerminalStatus::Partial);
    }
    #[test]
    fn failed_scope_does_not_claim_completion_and_handoff_keeps_running() {
        let op = start();
        drop(op.guard());
        assert_eq!(op.snapshot().status, TerminalStatus::Failed);
        let transferred = start();
        transferred.guard().handoff();
        assert_eq!(transferred.snapshot().status, TerminalStatus::Running);
        transferred.finish(false);
        assert_eq!(transferred.snapshot().status, TerminalStatus::Complete);
    }
    #[test]
    fn injected_unwind_abandons_only_its_operation() {
        let survivor = start();
        let failed = start();
        let _ = std::panic::catch_unwind(|| {
            let _guard = failed.guard();
            panic!("isolated operation injection");
        });
        assert_eq!(failed.snapshot().status, TerminalStatus::Abandoned);
        assert_eq!(survivor.snapshot().status, TerminalStatus::Running);
        survivor.finish(false);
    }
    #[test]
    fn operation_ids_are_distinct_from_feature_request_ids() {
        let first = start();
        let second = start();
        assert_ne!(
            first.snapshot().operation_id,
            second.snapshot().operation_id
        );
        assert_eq!(first.snapshot().request_id, second.snapshot().request_id);
        first.finish(false);
        second.finish(false);
    }
    #[test]
    fn full_active_history_refuses_registration_without_evicting_work() {
        let operation = start();
        let mut history = VecDeque::from(vec![operation.clone(); 64]);
        assert_eq!(
            reserve_history(&mut history).unwrap_err().code,
            "operationLimit"
        );
        assert_eq!(history.len(), 64);
        assert_eq!(operation.snapshot().status, TerminalStatus::Running);
        operation.finish(false);
        reserve_history(&mut history).unwrap();
        assert_eq!(history.len(), 63);
    }
    #[test]
    fn abandoned_read_watchdog_cancels_io_and_cannot_claim_later_completion() {
        let operation =
            Operation::start("duplicates", Some(9), Arc::new(AtomicBool::new(false))).unwrap();
        operation
            .start_readonly_watchdog(Duration::from_millis(10))
            .unwrap();
        let started = Instant::now();
        while (operation.snapshot().status != TerminalStatus::Abandoned
            || !operation.cancel.load(Ordering::Relaxed))
            && started.elapsed() < Duration::from_secs(1)
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(operation.snapshot().status, TerminalStatus::Abandoned);
        assert!(operation.cancel.load(Ordering::Relaxed));
        operation.finish(false);
        assert_eq!(operation.snapshot().status, TerminalStatus::Abandoned);
    }
    #[test]
    fn watchdogs_cannot_abandon_mutations_and_chunk_activity_is_observed() {
        let mutation = start();
        assert!(
            mutation
                .start_readonly_watchdog(Duration::from_millis(1))
                .is_err()
        );
        std::thread::sleep(Duration::from_millis(3));
        touch_token(&mutation.cancel);
        assert!(mutation.activity.last_ms.load(Ordering::Relaxed) > 0);
        mutation.finish(false);
    }
    #[test]
    fn notification_callbacks_can_read_status_without_lock_reentrancy() {
        let operation = start();
        let observer = operation.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let output = seen.clone();
        operation.set_sink(Arc::new(move |status| {
            assert_eq!(observer.snapshot().status, status.status);
            lock(&output).push(status.status);
        }));
        cancellation_requested(&operation.cancel);
        operation.finish(false);
        assert_eq!(
            *lock(&seen),
            vec![
                TerminalStatus::Running,
                TerminalStatus::CancelRequested,
                TerminalStatus::Partial
            ]
        );
        operation.set_sink(Arc::new(|_| {}));
    }
}
