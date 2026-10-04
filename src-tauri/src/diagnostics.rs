//! Shared, bounded diagnostics for read-only filesystem operations.
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct Diagnostics {
    pub unreadable: AtomicU64,
    pub changed: AtomicU64,
    pub excluded: AtomicU64,
    pub depth_limited: AtomicU64,
    pub resource_limited: AtomicU64,
    pub retained_bytes: AtomicU64,
    pub candidates: AtomicU64,
}

#[derive(Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub unreadable: u64,
    pub changed: u64,
    pub excluded: u64,
    pub depth_limited: u64,
    pub resource_limited: u64,
    pub coverage: &'static str,
}

impl Diagnostics {
    pub fn snapshot(&self) -> Snapshot {
        let mut result = Snapshot {
            unreadable: self.unreadable.load(Ordering::Relaxed),
            changed: self.changed.load(Ordering::Relaxed),
            excluded: self.excluded.load(Ordering::Relaxed),
            depth_limited: self.depth_limited.load(Ordering::Relaxed),
            resource_limited: self.resource_limited.load(Ordering::Relaxed),
            coverage: "complete",
        };
        if result.unreadable + result.changed + result.depth_limited + result.resource_limited > 0 {
            result.coverage = "partial";
        }
        result
    }

    pub fn error(&self, error: &std::io::Error) {
        if error.kind() == std::io::ErrorKind::OutOfMemory {
            self.resource_limited.fetch_add(1, Ordering::Relaxed);
        } else if error.kind() == std::io::ErrorKind::InvalidData {
            self.changed.fetch_add(1, Ordering::Relaxed);
        } else if error.kind() != std::io::ErrorKind::Interrupted {
            self.unreadable.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Enumeration is serial. Reserve before allocating paths/candidates.
    pub fn reserve_candidate(&self, bytes: u64, max_count: u64, max_bytes: u64) -> bool {
        let count = self.candidates.load(Ordering::Relaxed);
        let retained = self.retained_bytes.load(Ordering::Relaxed);
        if count >= max_count || retained.saturating_add(bytes) > max_bytes {
            self.resource_limited.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        self.candidates.store(count + 1, Ordering::Relaxed);
        self.retained_bytes
            .store(retained + bytes, Ordering::Relaxed);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusions_are_intentional_but_io_and_limits_make_results_partial() {
        let diagnostics = Diagnostics::default();
        diagnostics.excluded.store(9, Ordering::Relaxed);
        assert_eq!(diagnostics.snapshot().coverage, "complete");
        diagnostics.error(&std::io::ErrorKind::PermissionDenied.into());
        diagnostics.error(&std::io::ErrorKind::InvalidData.into());
        diagnostics.error(&std::io::ErrorKind::Interrupted.into());
        let snapshot = diagnostics.snapshot();
        assert_eq!((snapshot.unreadable, snapshot.changed), (1, 1));
        assert_eq!(snapshot.coverage, "partial");
    }

    #[test]
    fn candidate_budget_is_bounded_before_allocation() {
        let diagnostics = Diagnostics::default();
        assert!(diagnostics.reserve_candidate(60, 3, 100));
        assert!(!diagnostics.reserve_candidate(60, 3, 100));
        assert!(diagnostics.reserve_candidate(40, 3, 100));
        assert!(!diagnostics.reserve_candidate(1, 3, 100));
        assert_eq!(diagnostics.retained_bytes.load(Ordering::Relaxed), 100);
        assert_eq!(diagnostics.snapshot().resource_limited, 2);
    }
}
