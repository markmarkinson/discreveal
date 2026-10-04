//! Pure filtering, size limits and media concurrency policy. No filesystem calls or IPC.
use std::path::Path;
pub(crate) const NVME_WORKERS: usize = 12;
pub(crate) const SSD_WORKERS: usize = 6;
pub(crate) const USB_WORKERS: usize = 2;
pub(crate) const HDD_WORKERS: usize = 1;

/// Immutable filter settings of one scan.
pub(crate) struct ScanFilter {
    pub(crate) excludes_lower: Vec<String>,
    pub(crate) min_size_bytes: u64,
    pub(crate) max_size_bytes: u64,
}

impl ScanFilter {
    /// The parent path is identical for every entry. Resolve its part of each
    /// exclusion once, leaving exact filename comparisons in the hot loop.
    pub(crate) fn for_directory(&self, dir: &Path) -> DirectoryFilter<'_> {
        directory_exclusions(dir, &self.excludes_lower)
    }

    /// Whether a directory of `size` bytes should be kept as a childless stub.
    pub(crate) fn is_out_of_range(&self, size: u64) -> bool {
        (self.min_size_bytes > 0 && size < self.min_size_bytes)
            || (self.max_size_bytes > 0 && size > self.max_size_bytes)
    }
}

pub(crate) fn directory_exclusions<'a>(
    dir: &Path,
    excludes_lower: &'a [String],
) -> DirectoryFilter<'a> {
    if excludes_lower.is_empty() {
        return DirectoryFilter {
            exclude_all: false,
            leaf_names: Vec::new(),
        };
    }
    let prefix = dir.join("");
    let prefix = prefix.to_string_lossy();
    let lower = if prefix.is_ascii() {
        prefix.to_ascii_lowercase()
    } else {
        prefix.to_lowercase()
    };
    let mut leaf_names = Vec::new();
    for fragment in excludes_lower {
        // Includes fragments ending in a separator, which only exclude
        // the contents of a matching parent, not the parent entry itself.
        if mark_markinson_contains_at_boundaries(&lower, fragment) {
            return DirectoryFilter {
                exclude_all: true,
                leaf_names: Vec::new(),
            };
        }
        if let Some(last_separator) = fragment.rfind('\\') {
            let (required_parent, leaf) = fragment.split_at(last_separator + 1);
            if leaf.is_empty() || !lower.ends_with(required_parent) {
                continue;
            }
            let start = lower.len() - required_parent.len();
            if start == 0 || fragment.starts_with('\\') || lower[..start].ends_with('\\') {
                leaf_names.push(leaf);
            }
        } else {
            leaf_names.push(fragment.as_str());
        }
    }
    DirectoryFilter {
        exclude_all: false,
        leaf_names,
    }
}

/// Whether `path` matches any of `excludes_lower` (already-lowercased
/// fragments), as whole path components. Runs once per file *and* per
/// directory a general walk finds — on a multi-million-file drive this is called
/// well over a million times — so the common case (an all-ASCII path, which
/// is virtually every real Windows path) is lowercased with a cheap
/// byte-level ASCII transform instead of the full Unicode case-folding
/// `str::to_lowercase` does; the actual substring search below is unchanged
/// (`str::find`, which is SIMD-accelerated in std) either way, since that
/// turned out to be the part worth not touching.
///
/// An earlier version of this function also tried to avoid the allocation
/// entirely with a hand-written byte-by-byte search — measured, not assumed,
/// against a real ~1.5M-file `C:` scan, and it made the whole scan **~70%
/// slower** (consistently, across repeated runs: ~25-29s before against
/// ~44-46s with the hand-written search), not faster. `str::find`'s
/// SIMD/memchr-backed implementation so thoroughly outweighs one extra
/// allocation per file that a naive nested-loop substring search lost
/// despite allocating nothing. Keeping `str::find` and only cutting the
/// *lowercasing* cost (full Unicode case folding vs. a flat ASCII byte
/// transform) is the version that actually measured faster — see the
/// decision record in `.internal/STATE.md` before changing this again.
/// The drive scan now resolves the parent portion once in `for_directory`;
/// this general matcher remains the reference implementation and is also
/// used by the duplicate finder.
#[allow(dead_code)] // Reference matcher for differential tests and scanner benchmarks.
pub(crate) fn is_excluded(path: &Path, excludes_lower: &[String]) -> bool {
    if excludes_lower.is_empty() {
        return false;
    }
    let path_str = path.to_string_lossy();
    let lower = if path_str.is_ascii() {
        path_str.to_ascii_lowercase()
    } else {
        path_str.to_lowercase()
    };
    excludes_lower
        .iter()
        .any(|fragment| mark_markinson_contains_at_boundaries(&lower, fragment))
}

/// Whether `fragment` occurs in `path` as whole path components, so that
/// excluding `d:\data` does not also exclude `d:\database`. `path` is
/// expected to already be lowercase — see [`is_excluded`], the one real
/// call site, which lowercases once up front rather than per comparison.
pub(crate) fn mark_markinson_contains_at_boundaries(path: &str, fragment: &str) -> bool {
    let mut from = 0;
    while let Some(offset) = path[from..].find(fragment) {
        let start = from + offset;
        let end = start + fragment.len();
        let starts_on_boundary =
            start == 0 || fragment.starts_with('\\') || path[..start].ends_with('\\');
        let ends_on_boundary =
            end == path.len() || fragment.ends_with('\\') || path[end..].starts_with('\\');
        if starts_on_boundary && ends_on_boundary {
            return true;
        }
        from = start + path[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Counters and flags shared by every thread of one scan.
pub(crate) struct DirectoryFilter<'a> {
    pub(crate) exclude_all: bool,
    leaf_names: Vec<&'a str>,
}

impl DirectoryFilter<'_> {
    pub(crate) fn has_leaf_exclusions(&self) -> bool {
        !self.leaf_names.is_empty()
    }
    pub(crate) fn is_excluded(&self, name: &str) -> bool {
        if self.exclude_all {
            return true;
        }
        if name.is_ascii() {
            self.leaf_names
                .iter()
                .any(|leaf| name.eq_ignore_ascii_case(leaf))
        } else if self.leaf_names.is_empty() {
            false
        } else {
            let lower = name.to_lowercase();
            self.leaf_names.iter().any(|leaf| lower == *leaf)
        }
    }
}

pub(crate) fn worker_count_for_drive(bus_type: &str, is_ssd: bool) -> usize {
    if bus_type.eq_ignore_ascii_case("nvme") {
        NVME_WORKERS
    } else if bus_type.eq_ignore_ascii_case("usb") {
        USB_WORKERS
    } else if is_ssd {
        SSD_WORKERS
    } else {
        // A single reader keeps the head movement sequential on spinning disks.
        HDD_WORKERS
    }
}

/// Converts a user-supplied limit to bytes; negative or non-finite values
/// disable the limit.
pub(crate) fn limit_to_bytes(value: f64, unit: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        (value * unit) as u64
    } else {
        0
    }
}
