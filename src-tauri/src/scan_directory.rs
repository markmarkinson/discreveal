//! Read-only directory enumeration for the drive scan. Windows' large-fetch
//! buffer reduces query round trips when consuming an entire listing.
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub(super) enum DirectoryEntry {
    Standard(fs::DirEntry),
    #[cfg(windows)]
    Native {
        root: std::sync::Arc<PathBuf>,
        data: windows_sys::Win32::Storage::FileSystem::WIN32_FIND_DATAW,
    },
}

pub(super) struct EntryType {
    directory: bool,
    file: bool,
    symlink: bool,
}

impl EntryType {
    pub(super) fn is_dir(&self) -> bool {
        self.directory
    }
    pub(super) fn is_file(&self) -> bool {
        self.file
    }
    pub(super) fn is_symlink(&self) -> bool {
        self.symlink
    }
}

impl DirectoryEntry {
    /// Cleanup needs all reparse attributes, including cloud placeholders.
    #[cfg(windows)]
    pub(super) fn cleanup_details(&self) -> io::Result<(bool, u32, u64)> {
        use std::os::windows::fs::MetadataExt;
        match self {
            Self::Standard(entry) => fs::symlink_metadata(entry.path()).map(|meta| {
                (
                    meta.is_dir(),
                    meta.file_attributes(),
                    meta.last_write_time(),
                )
            }),
            Self::Native { data, .. } => Ok((
                data.dwFileAttributes
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY
                    != 0,
                data.dwFileAttributes,
                ((data.ftLastWriteTime.dwHighDateTime as u64) << 32)
                    | data.ftLastWriteTime.dwLowDateTime as u64,
            )),
        }
    }

    pub(super) fn path(&self) -> PathBuf {
        match self {
            Self::Standard(entry) => entry.path(),
            #[cfg(windows)]
            Self::Native { root, .. } => root.join(self.file_name()),
        }
    }

    pub(super) fn file_name(&self) -> OsString {
        match self {
            Self::Standard(entry) => entry.file_name(),
            #[cfg(windows)]
            Self::Native { data, .. } => {
                use std::os::windows::ffi::OsStringExt;
                let end = data
                    .cFileName
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(data.cFileName.len());
                OsString::from_wide(&data.cFileName[..end])
            }
        }
    }

    pub(super) fn file_type(&self) -> io::Result<EntryType> {
        match self {
            Self::Standard(entry) => entry.file_type().map(|kind| EntryType {
                directory: kind.is_dir(),
                file: kind.is_file(),
                symlink: kind.is_symlink(),
            }),
            #[cfg(windows)]
            Self::Native { data, .. } => {
                use windows_sys::Win32::Storage::FileSystem::{
                    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
                };
                // Match std: skip only name-surrogate reparse points (junctions,
                // symlinks), not cloud placeholders or other ordinary files.
                let symlink = data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                    && data.dwReserved0 & 0x20000000 != 0;
                let directory = data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
                Ok(EntryType {
                    directory: directory && !symlink,
                    file: !directory && !symlink,
                    symlink,
                })
            }
        }
    }

    pub(super) fn file_details(&self) -> (u64, u64, bool) {
        let (size, modified, denied) = self.file_details_precise();
        (size, modified / 1_000_000_000, denied)
    }

    /// Nanosecond timestamps also invalidate duplicate samples after same-second edits.
    pub(super) fn file_details_precise(&self) -> (u64, u64, bool) {
        match self {
            Self::Standard(entry) => match entry.metadata() {
                Ok(metadata) => (
                    metadata.len(),
                    metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map_or(0, |duration| {
                            u64::try_from(duration.as_nanos()).unwrap_or(0)
                        }),
                    false,
                ),
                Err(_) => (0, 0, true),
            },
            #[cfg(windows)]
            Self::Native { data, .. } => {
                let size = ((data.nFileSizeHigh as u64) << 32) | data.nFileSizeLow as u64;
                let ticks = ((data.ftLastWriteTime.dwHighDateTime as u64) << 32)
                    | data.ftLastWriteTime.dwLowDateTime as u64;
                (
                    size,
                    ticks
                        .saturating_sub(116_444_736_000_000_000)
                        .checked_mul(100)
                        .unwrap_or(0),
                    false,
                )
            }
        }
    }
}

pub(super) enum ReadDirectory {
    Standard(fs::ReadDir),
    Ordered(std::vec::IntoIter<io::Result<DirectoryEntry>>),
    #[cfg(windows)]
    Native(native::ReadDirectory),
}

pub(super) fn read_dir(path: &Path) -> io::Result<ReadDirectory> {
    #[cfg(windows)]
    if let Ok(reader) = native::ReadDirectory::open(path) {
        return Ok(ReadDirectory::Native(reader));
    }
    // Unsupported flags/filesystems, empty directories and open failures keep
    // std's established behavior. Never turn an error into a successful scan.
    fs::read_dir(path).map(ReadDirectory::Standard)
}

/// Duplicate scans discard most files by size. Standard enumeration measured
/// faster for that metadata-only workload; preserve native enumeration for the drive scan.
pub(super) fn read_dir_standard(path: &Path) -> io::Result<ReadDirectory> {
    fs::read_dir(path).map(ReadDirectory::Standard)
}

impl Iterator for ReadDirectory {
    type Item = io::Result<DirectoryEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ordered(reader) => reader.next(),
            Self::Standard(reader) => reader
                .next()
                .map(|entry| entry.map(DirectoryEntry::Standard)),
            #[cfg(windows)]
            Self::Native(reader) => reader.next(),
        }
    }
}

impl ReadDirectory {
    /// Precise accounting attributes hardlinks to the first path in this
    /// ordered traversal. Fast scans keep streaming without collecting entries.
    pub(super) fn ordered(self, precise: bool) -> Self {
        if !precise {
            return self;
        }
        let mut entries: Vec<_> = self.take(65_537).collect();
        let limited = entries.len() > 65_536;
        if limited {
            entries.truncate(65_536);
        }
        entries.sort_by_cached_key(|entry| entry.as_ref().ok().map(DirectoryEntry::file_name));
        if limited {
            entries.push(Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "ordered directory limit",
            )));
        }
        Self::Ordered(entries.into_iter())
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        FIND_FIRST_EX_LARGE_FETCH, FindClose, FindExInfoBasic, FindExSearchNameMatch,
        FindFirstFileExW, FindNextFileW, WIN32_FIND_DATAW,
    };

    pub(crate) struct ReadDirectory {
        handle: HANDLE,
        root: Arc<PathBuf>,
        first: Option<WIN32_FIND_DATAW>,
    }

    impl ReadDirectory {
        pub(super) fn open(path: &Path) -> io::Result<Self> {
            if path.as_os_str().is_empty() {
                return Err(io::ErrorKind::NotFound.into());
            }
            let absolute = std::path::absolute(path)?;
            let pattern = absolute.join("*");
            let wide: Vec<u16> = pattern.as_os_str().encode_wide().collect();
            if wide.contains(&0) {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            // Verbatim paths retain long names without changing the result's
            // displayed paths. All drive scan roots have already been validated.
            let mut query = if wide.starts_with(&[92, 92, 63, 92]) {
                wide
            } else if wide.starts_with(&[92, 92]) {
                "\\\\?\\UNC\\"
                    .encode_utf16()
                    .chain(wide.into_iter().skip(2))
                    .collect()
            } else {
                "\\\\?\\".encode_utf16().chain(wide).collect()
            };
            query.push(0);
            let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
            // SAFETY: query is NUL-terminated, data is a writable correctly sized
            // WIN32_FIND_DATAW, no filters. This object owns the returned handle.
            let handle = unsafe {
                FindFirstFileExW(
                    query.as_ptr(),
                    FindExInfoBasic,
                    (&mut data as *mut WIN32_FIND_DATAW).cast(),
                    FindExSearchNameMatch,
                    std::ptr::null(),
                    FIND_FIRST_EX_LARGE_FETCH,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                handle,
                root: Arc::new(path.to_path_buf()),
                first: Some(data),
            })
        }

        fn close(&mut self) {
            if self.handle != INVALID_HANDLE_VALUE {
                // SAFETY: handle came from FindFirstFileExW; invalidate it after
                // closing to prevent a second close on end-of-listing or Drop.
                unsafe {
                    FindClose(self.handle);
                }
                self.handle = INVALID_HANDLE_VALUE;
            }
        }
    }

    impl Iterator for ReadDirectory {
        type Item = io::Result<DirectoryEntry>;
        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if self.handle == INVALID_HANDLE_VALUE {
                    return None;
                }
                let data = if let Some(first) = self.first.take() {
                    first
                } else {
                    let mut data = unsafe { std::mem::zeroed() };
                    // SAFETY: live owned enumeration handle and writable buffer.
                    if unsafe { FindNextFileW(self.handle, &mut data) } == 0 {
                        let error = io::Error::last_os_error();
                        self.close();
                        return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                            None
                        } else {
                            Some(Err(error))
                        };
                    }
                    data
                };
                let name = &data.cFileName;
                if name[0] == 46 && (name[1] == 0 || (name[1] == 46 && name[2] == 0)) {
                    continue;
                }
                return Some(Ok(DirectoryEntry::Native {
                    root: Arc::clone(&self.root),
                    data,
                }));
            }
        }
    }

    impl Drop for ReadDirectory {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "discreveal-enumeration-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            Self(fs::canonicalize(root).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    type EntrySnapshot = BTreeMap<OsString, (PathBuf, bool, bool, bool, (u64, u64, bool))>;
    fn snapshot(entries: impl Iterator<Item = io::Result<DirectoryEntry>>) -> EntrySnapshot {
        entries
            .map(|entry| {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                let details = if kind.is_file() {
                    entry.file_details()
                } else {
                    (0, 0, false)
                };
                (
                    entry.file_name(),
                    (
                        entry.path(),
                        kind.is_dir(),
                        kind.is_file(),
                        kind.is_symlink(),
                        details,
                    ),
                )
            })
            .collect()
    }

    #[test]
    fn enumeration_matches_standard_names_paths_sizes_and_timestamps() {
        let fixture = Fixture::new();
        for name in ["plain.txt", "Äpfel 日本語 🦀.bin", "zero", " spaces .txt"] {
            fs::write(
                fixture.0.join(name),
                if name == "zero" {
                    b"".as_slice()
                } else {
                    b"discReveal by markMarkinson".as_slice()
                },
            )
            .unwrap();
        }
        fs::create_dir(fixture.0.join("Unterordner 日本語")).unwrap();
        let actual = snapshot(read_dir(&fixture.0).unwrap());
        let expected = snapshot(
            fs::read_dir(&fixture.0)
                .unwrap()
                .map(|entry| entry.map(DirectoryEntry::Standard)),
        );
        assert_eq!(actual, expected);
        #[cfg(windows)]
        assert!(matches!(
            read_dir(&fixture.0).unwrap(),
            ReadDirectory::Native(_)
        ));
    }

    #[test]
    fn empty_and_missing_directories_remain_distinct() {
        let fixture = Fixture::new();
        assert_eq!(read_dir(&fixture.0).unwrap().count(), 0);
        assert!(read_dir(&fixture.0.join("missing")).is_err());
    }

    #[test]
    fn long_paths_enumerate_without_truncating() {
        let fixture = Fixture::new();
        let mut long = fixture.0.clone();
        for _ in 0..12 {
            long.push("long-directory-component");
        }
        fs::create_dir_all(&long).unwrap();
        fs::write(long.join("日本語.txt"), b"long path").unwrap();
        assert_eq!(
            snapshot(read_dir(&long).unwrap()),
            snapshot(
                fs::read_dir(&long)
                    .unwrap()
                    .map(|entry| entry.map(DirectoryEntry::Standard))
            )
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_metadata_keeps_64_bit_sizes_and_reparse_semantics() {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, WIN32_FIND_DATAW,
        };
        let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
        data.nFileSizeHigh = 3;
        data.nFileSizeLow = 42;
        let ticks = 116_444_736_000_000_000u64 + 1_700_000_000 * 10_000_000;
        data.ftLastWriteTime.dwHighDateTime = (ticks >> 32) as u32;
        data.ftLastWriteTime.dwLowDateTime = ticks as u32;
        let mut entry = DirectoryEntry::Native {
            root: std::sync::Arc::new(PathBuf::from("C:\\")),
            data,
        };
        assert_eq!(
            entry.file_details(),
            ((3u64 << 32) + 42, 1_700_000_000, false)
        );
        if let DirectoryEntry::Native { data, .. } = &mut entry {
            data.dwFileAttributes = FILE_ATTRIBUTE_REPARSE_POINT;
            data.dwReserved0 = 0x9000001a; // Cloud placeholder: a file, not a link.
        }
        assert!(entry.file_type().unwrap().is_file());
        if let DirectoryEntry::Native { data, .. } = &mut entry {
            data.dwFileAttributes |= FILE_ATTRIBUTE_DIRECTORY;
            data.dwReserved0 = 0xa0000003; // Junction: must never recurse.
        }
        let kind = entry.file_type().unwrap();
        assert!(kind.is_symlink());
        assert!(!kind.is_dir());
        assert!(!kind.is_file());
    }
}
