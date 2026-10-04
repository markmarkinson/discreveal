//! Read-only, handle-bound storage accounting. Logical length remains separate
//! from allocated data-stream bytes; filesystem metadata and snapshots are not
//! attributed to individual files. No data is recalled from cloud placeholders.
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

pub(crate) type Identity = (u64, [u8; 16]);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Measurement {
    pub identity: Identity,
    pub logical: u64,
    pub allocated: u64,
    pub links: u32,
}

/// Only multiply linked objects need retention. Bound both entries and path
/// storage; reaching this limit is an explicit incomplete measurement.
#[derive(Default)]
pub(crate) struct Ledger {
    owners: Mutex<(HashMap<Identity, PathBuf>, usize)>,
}

impl Ledger {
    pub(crate) fn charge(&self, value: Measurement, path: &Path) -> io::Result<(u64, bool)> {
        if value.links <= 1 {
            return Ok((value.allocated, false));
        }
        let mut owners = self.owners.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(owner) = owners.0.get(&value.identity) {
            return Ok((
                if owner == path { value.allocated } else { 0 },
                owner != path,
            ));
        }
        let cost = path.as_os_str().len().saturating_mul(2).saturating_add(96);
        if owners.0.len() >= 250_000 || owners.1.saturating_add(cost) > 64 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "hardlink accounting limit",
            ));
        }
        owners.1 += cost;
        owners.0.insert(value.identity, path.to_path_buf());
        Ok((value.allocated, false))
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::fs::OpenOptions;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_HANDLE_EOF, ERROR_MORE_DATA};
    use windows_sys::Win32::Storage::FileSystem::*;

    fn query<T>(file: &File, class: FILE_INFO_BY_HANDLE_CLASS) -> io::Result<T> {
        let mut info = std::mem::MaybeUninit::<T>::zeroed();
        // SAFETY: owned live handle; class and output type are paired below.
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle().cast(),
                class,
                info.as_mut_ptr().cast(),
                std::mem::size_of::<T>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful API call initialized the matching structure.
        Ok(unsafe { info.assume_init() })
    }

    fn open(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL)
            .open(path)
    }

    fn identity(file: &File) -> io::Result<Identity> {
        let id: FILE_ID_INFO = query(file, FileIdInfo)?;
        if id.FileId.Identifier == [0; 16] {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "file identity unavailable",
            ));
        }
        Ok((id.VolumeSerialNumber, id.FileId.Identifier))
    }

    fn stream_allocation(file: &File) -> io::Result<u64> {
        let standard: FILE_STANDARD_INFO = query(file, FileStandardInfo)?;
        if standard.AllocationSize < 0 || standard.DeletePending {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let attributes = file.metadata()?.file_attributes();
        if attributes & (FILE_ATTRIBUTE_COMPRESSED | FILE_ATTRIBUTE_SPARSE_FILE) != 0 {
            // AllocationSize may describe virtual allocation for these files.
            let compressed: FILE_COMPRESSION_INFO = query(file, FileCompressionInfo)?;
            u64::try_from(compressed.CompressedFileSize)
                .map_err(|_| io::ErrorKind::InvalidData.into())
        } else {
            Ok(standard.AllocationSize as u64)
        }
    }

    fn stream_names(file: &File) -> io::Result<Vec<String>> {
        let mut buffer = vec![0u64; 512];
        loop {
            // SAFETY: u64 backing provides alignment and the full writable size.
            if unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle().cast(),
                    FileStreamInfo,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * 8) as u32,
                )
            } != 0
            {
                break;
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_HANDLE_EOF as i32) {
                return Ok(Vec::new());
            }
            if error.raw_os_error() != Some(ERROR_MORE_DATA as i32) || buffer.len() >= 16_384 {
                return Err(error);
            }
            buffer.resize(buffer.len() * 2, 0);
        }
        let mut names = Vec::new();
        let mut offset = 0usize;
        let capacity = buffer.len() * 8;
        let name_offset = std::mem::offset_of!(FILE_STREAM_INFO, StreamName);
        loop {
            if offset
                .checked_add(std::mem::size_of::<FILE_STREAM_INFO>())
                .is_none_or(|end| end > capacity)
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // SAFETY: bounds checked above; Windows supplied this record.
            let record = unsafe {
                std::ptr::read_unaligned(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_STREAM_INFO>(),
                )
            };
            let length = record.StreamNameLength as usize;
            if !length.is_multiple_of(2) || offset + name_offset + length > capacity {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // SAFETY: UTF-16 name bounds and even length were validated.
            let chars = unsafe {
                std::slice::from_raw_parts(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset + name_offset)
                        .cast::<u16>(),
                    length / 2,
                )
            };
            names.push(
                String::from_utf16(chars)
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
            );
            if record.NextEntryOffset == 0 {
                break;
            }
            let next = record.NextEntryOffset as usize;
            if next < name_offset + length || !next.is_multiple_of(8) {
                return Err(io::ErrorKind::InvalidData.into());
            }
            offset = offset.checked_add(next).ok_or(io::ErrorKind::InvalidData)?;
        }
        Ok(names)
    }

    pub(super) fn from_file(file: &File, path: &Path) -> io::Result<Measurement> {
        let before = file.metadata()?;
        if !before.is_file()
            || before.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE)
                != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "placeholder/reparse storage unavailable",
            ));
        }
        let id = identity(file)?;
        let standard: FILE_STANDARD_INFO = query(file, FileStandardInfo)?;
        let mut allocated = stream_allocation(file)?;
        // Stream info is unsupported on some filesystems. Report that rather
        // than silently assert a complete per-file allocation measurement.
        for name in stream_names(file)? {
            if name == "::$DATA" {
                continue;
            }
            if !name.starts_with(':')
                || !name.ends_with(":$DATA")
                || name.contains(['\\', '/', '\0'])
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut stream_path = path.as_os_str().to_os_string();
            stream_path.push(&name);
            let stream = open(Path::new(&stream_path))?;
            if identity(&stream)? != id {
                return Err(io::ErrorKind::InvalidData.into());
            }
            allocated = allocated
                .checked_add(stream_allocation(&stream)?)
                .ok_or(io::ErrorKind::InvalidData)?;
        }
        let after = file.metadata()?;
        let final_info: FILE_STANDARD_INFO = query(file, FileStandardInfo)?;
        if before.len() != after.len()
            || before.last_write_time() != after.last_write_time()
            || standard.NumberOfLinks != final_info.NumberOfLinks
            || final_info.DeletePending
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Measurement {
            identity: id,
            logical: after.len(),
            allocated,
            links: final_info.NumberOfLinks,
        })
    }

    pub(super) fn measure(path: &Path) -> io::Result<Measurement> {
        from_file(&open(path)?, path)
    }
}

#[cfg(windows)]
pub(crate) fn measure(path: &Path) -> io::Result<Measurement> {
    native::measure(path)
}
#[cfg(windows)]
pub(crate) fn from_file(file: &File, path: &Path) -> io::Result<Measurement> {
    native::from_file(file, path)
}

#[cfg(not(windows))]
pub(crate) fn measure(_path: &Path) -> io::Result<Measurement> {
    Err(io::ErrorKind::Unsupported.into())
}
#[cfg(not(windows))]
pub(crate) fn from_file(_file: &File, _path: &Path) -> io::Result<Measurement> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardlinks_are_charged_once_and_rereads_keep_the_same_owner() {
        let ledger = Ledger::default();
        let value = Measurement {
            identity: (7, [1; 16]),
            logical: 1,
            allocated: 4096,
            links: 2,
        };
        assert_eq!(
            ledger.charge(value, Path::new("first")).unwrap(),
            (4096, false)
        );
        assert_eq!(
            ledger.charge(value, Path::new("second")).unwrap(),
            (0, true)
        );
        assert_eq!(
            ledger.charge(value, Path::new("first")).unwrap(),
            (4096, false)
        );
        let other_volume = Measurement {
            identity: (8, [1; 16]),
            ..value
        };
        assert_eq!(
            ledger.charge(other_volume, Path::new("other")).unwrap(),
            (4096, false)
        );
    }
    #[test]
    fn accounting_limit_is_an_error_instead_of_a_false_zero() {
        let ledger = Ledger::default();
        ledger.owners.lock().unwrap().1 = 64 * 1024 * 1024;
        let value = Measurement {
            identity: (7, [1; 16]),
            logical: 1,
            allocated: 4096,
            links: 2,
        };
        assert_eq!(
            ledger.charge(value, Path::new("file")).unwrap_err().kind(),
            io::ErrorKind::OutOfMemory
        );
    }

    #[cfg(windows)]
    struct Fixture(PathBuf);
    #[cfg(windows)]
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("discreveal-allocation-{}", rand::random::<u64>()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    #[cfg(windows)]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn real_allocation_counts_clusters_hardlinks_and_named_streams() {
        let fixture = Fixture::new();
        let path = fixture.0.join("data.bin");
        std::fs::write(&path, vec![7; 8193]).unwrap();
        let plain = measure(&path).unwrap();
        assert_eq!(plain.logical, 8193);
        assert!(
            plain.allocated > plain.logical,
            "non-cluster-aligned file has cluster overhead"
        );
        let link = fixture.0.join("alias.bin");
        std::fs::hard_link(&path, &link).unwrap();
        let alias = measure(&link).unwrap();
        assert_eq!(plain.identity, alias.identity);
        assert_eq!(alias.links, 2);
        let ledger = Ledger::default();
        assert_eq!(ledger.charge(alias, &path).unwrap().0, plain.allocated);
        assert_eq!(ledger.charge(alias, &link).unwrap(), (0, true));
        let mut stream = path.as_os_str().to_os_string();
        stream.push(":extra");
        std::fs::write(Path::new(&stream), vec![8; 8193]).unwrap();
        let with_stream = measure(&path).unwrap();
        assert_eq!(with_stream.logical, plain.logical);
        assert!(
            with_stream.allocated >= plain.allocated * 2,
            "named stream allocation must be included"
        );
    }

    #[cfg(windows)]
    fn control(
        file: &File,
        code: u32,
        input: *const std::ffi::c_void,
        length: u32,
    ) -> io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        let mut returned = 0;
        // SAFETY: caller supplies the matching initialized FSCTL input; live
        // owned fixture handle, no output and synchronous call.
        if unsafe {
            windows_sys::Win32::System::IO::DeviceIoControl(
                file.as_raw_handle().cast(),
                code,
                input,
                length,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn real_sparse_and_compressed_allocation_differ_from_logical_length() {
        use std::io::{Seek, SeekFrom, Write};
        use windows_sys::Win32::System::Ioctl::{FSCTL_SET_COMPRESSION, FSCTL_SET_SPARSE};
        let fixture = Fixture::new();
        let sparse_path = fixture.0.join("sparse.bin");
        let mut sparse = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&sparse_path)
            .unwrap();
        control(&sparse, FSCTL_SET_SPARSE, std::ptr::null(), 0).unwrap();
        sparse.set_len(64 * 1024 * 1024).unwrap();
        sparse.seek(SeekFrom::Start(8 * 1024 * 1024)).unwrap();
        sparse.write_all(&vec![3; 65536]).unwrap();
        sparse.sync_all().unwrap();
        let measured = from_file(&sparse, &sparse_path).unwrap();
        assert_eq!(measured.logical, 64 * 1024 * 1024);
        assert!(measured.allocated > 0 && measured.allocated < measured.logical / 2);
        let compressed_path = fixture.0.join("compressed.bin");
        let mut compressed = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&compressed_path)
            .unwrap();
        let format: u16 = 1; // COMPRESSION_FORMAT_DEFAULT
        control(
            &compressed,
            FSCTL_SET_COMPRESSION,
            (&format as *const u16).cast(),
            2,
        )
        .unwrap();
        compressed.write_all(&vec![0; 256 * 1024]).unwrap();
        compressed.sync_all().unwrap();
        let measured = from_file(&compressed, &compressed_path).unwrap();
        assert_eq!(measured.logical, 256 * 1024);
        assert!(measured.allocated < measured.logical / 2);
    }
}
