//! Enumeration of local drives with their physical media type.
//!
//! Media and bus type decide how many scan workers are useful. They are read
//! with the Win32 storage APIs directly, which is fast, needs no elevated
//! rights and does not depend on PowerShell being available.

use crate::win::wide;
use serde::Serialize;
use std::mem::size_of;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, GetDiskFreeSpaceExW, GetDriveTypeW,
    GetLogicalDrives, GetVolumeInformationW, OPEN_EXISTING,
};
use windows_sys::Win32::System::Diagnostics::Debug::{SEM_FAILCRITICALERRORS, SetThreadErrorMode};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_PROPERTY_QUERY,
    StorageDeviceProperty, StorageDeviceSeekPenaltyProperty,
};

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const UNKNOWN: &str = "Unknown";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    path: String,
    total_size: u64,
    used_size: u64,
    is_removable: bool,
    file_system: String,
    media_type: String,
    bus_type: String,
    is_ssd: bool,
}

/// Capacity and free space in bytes, or `None` if the drive holds no media.
fn capacity(root: &[u16]) -> Option<(u64, u64)> {
    let (mut total, mut free) = (0u64, 0u64);
    // SAFETY: `root` is null-terminated and both out pointers are valid.
    let ok = unsafe { GetDiskFreeSpaceExW(root.as_ptr(), null_mut(), &mut total, &mut free) };
    (ok != 0 && total > 0).then_some((total, free))
}

fn file_system(root: &[u16]) -> String {
    let mut name = [0u16; 32];
    // SAFETY: `root` is null-terminated; `name` is a valid buffer of the stated length.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if ok == 0 {
        return UNKNOWN.to_string();
    }
    let length = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    match String::from_utf16_lossy(&name[..length]) {
        name if name.is_empty() => UNKNOWN.to_string(),
        name => name,
    }
}

/// Human-readable name of a `STORAGE_BUS_TYPE` value.
fn bus_type_name(bus_type: i32) -> &'static str {
    match bus_type {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        4 => "1394",
        6 => "Fibre Channel",
        7 => "USB",
        8 => "RAID",
        9 => "iSCSI",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        14 | 15 => "Virtual",
        16 => "Storage Spaces",
        17 => "NVMe",
        18 => "SCM",
        19 => "UFS",
        _ => UNKNOWN,
    }
}

/// Leading fields of `DEVICE_SEEK_PENALTY_DESCRIPTOR`. Flag bytes are read
/// as integers, since a driver-filled value other than 0 or 1 must not be
/// interpreted as a `bool`.
#[repr(C)]
#[derive(Clone, Copy)]
struct SeekPenaltyReply {
    version: u32,
    size: u32,
    incurs_seek_penalty: u8,
}

/// Leading fields of `STORAGE_DEVICE_DESCRIPTOR` up to the bus type.
#[repr(C)]
#[derive(Clone, Copy)]
struct DeviceDescriptorReply {
    version: u32,
    size: u32,
    device_type: u8,
    device_type_modifier: u8,
    removable_media: u8,
    command_queueing: u8,
    vendor_id_offset: u32,
    product_id_offset: u32,
    product_revision_offset: u32,
    serial_number_offset: u32,
    bus_type: i32,
}

/// Sends a standard storage property query to an open volume handle and
/// returns the reply if the driver answered. `T` must be valid for any bit
/// pattern, which holds for the plain-integer reply structs above.
fn query_storage_property<T: Copy>(volume: HANDLE, property_id: i32) -> Option<T> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: property_id,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    // Aligned scratch space, large enough for the variable-length descriptors.
    let mut reply = [0u64; 128];
    let mut returned = 0u32;
    // SAFETY: input and output buffers are valid for the stated lengths for the duration of the call.
    let ok = unsafe {
        DeviceIoControl(
            volume,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&raw const query).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            reply.as_mut_ptr().cast(),
            size_of_val(&reply) as u32,
            &mut returned,
            null_mut(),
        )
    };
    if ok == 0 || (returned as usize) < size_of::<T>() {
        return None;
    }
    // SAFETY: the driver filled at least `size_of::<T>()` bytes, `reply` is 8-byte aligned and `T` accepts any bit pattern.
    Some(unsafe { reply.as_ptr().cast::<T>().read() })
}

/// Media and bus type of the physical disk behind a drive letter, as
/// `(media type, bus type)`. Virtual and network drives report `Unknown`.
pub(crate) fn media_and_bus_type(letter: char) -> (&'static str, &'static str) {
    let path = wide(&format!(r"\\.\{letter}:"));
    // Zero access rights are enough for property queries, so no elevation is needed.
    // SAFETY: `path` is null-terminated; all other arguments are plain values.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return (UNKNOWN, UNKNOWN);
    }

    let media =
        query_storage_property::<SeekPenaltyReply>(handle, StorageDeviceSeekPenaltyProperty)
            .map_or(UNKNOWN, |reply| {
                if reply.incurs_seek_penalty == 0 {
                    "SSD"
                } else {
                    "HDD"
                }
            });
    let bus = query_storage_property::<DeviceDescriptorReply>(handle, StorageDeviceProperty)
        .map_or(UNKNOWN, |reply| bus_type_name(reply.bus_type));

    // SAFETY: `handle` was opened above and is closed exactly once.
    unsafe { CloseHandle(handle) };
    (media, bus)
}

/// Builds the entry for one drive letter, or `None` if it is not a local
/// fixed or removable drive that currently holds media.
fn describe_drive(letter: char) -> Option<Drive> {
    let path = format!("{letter}:\\");
    let root = wide(&path);

    // SAFETY: `root` is null-terminated.
    let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
    if drive_type != DRIVE_FIXED && drive_type != DRIVE_REMOVABLE {
        return None;
    }
    let (total_size, free_size) = capacity(&root)?;
    let is_removable = drive_type == DRIVE_REMOVABLE;
    let (media_type, bus_type) = media_and_bus_type(letter);

    Some(Drive {
        path,
        total_size,
        used_size: total_size.saturating_sub(free_size),
        is_removable,
        file_system: file_system(&root),
        // Removable media is scanned with a single reader regardless of type.
        is_ssd: media_type == "SSD" && !is_removable,
        media_type: media_type.to_string(),
        bus_type: bus_type.to_string(),
    })
}

/// Whether `path` lies on a fixed local drive, the only kind with a recycle bin.
pub fn is_fixed_drive(path: &Path) -> bool {
    let Some(letter) = path
        .to_str()
        .and_then(|text| text.chars().next())
        .filter(char::is_ascii_alphabetic)
    else {
        return false;
    };
    let root = wide(&format!("{letter}:\\"));
    // SAFETY: `root` is null-terminated.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_FIXED }
}

/// Lists local fixed and removable drives that currently hold media.
#[tauri::command(async)]
pub fn list_drives() -> Vec<Drive> {
    // Suppresses the "no disk in drive" system dialog for empty removable
    // drives, for this thread only.
    // SAFETY: plain flag argument; the previous mode is not requested.
    unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, null_mut()) };

    // SAFETY: no arguments.
    let letter_mask = unsafe { GetLogicalDrives() };
    ('A'..='Z')
        .enumerate()
        .filter(|(index, _)| letter_mask & (1 << index) != 0)
        .filter_map(|(_, letter)| describe_drive(letter))
        .collect()
}
