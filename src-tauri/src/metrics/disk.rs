//! ディスクの読み書き量。物理ドライブごとの累計を `IOCTL_DISK_PERFORMANCE` で取り、前回と比べる。
//!
//! 性能カウンターの `\PhysicalDisk` は、登録が壊れていると引けない。
//! ドライブに直接問い合わせれば、その影響を受けない。開くときに読み書きの権限は求めないので、管理者でなくても使える。

use super::rate::{keyed_delta, Rate};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Ioctl::{DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE};
use windows_sys::Win32::System::IO::DeviceIoControl;

/// 調べる物理ドライブの番号の上限
const MAX_DRIVES: u32 = 32;
/// USB ドライブの抜き差しに追いつくよう、ドライブを数え直す間隔
const RESCAN: Duration = Duration::from_secs(30);

struct Drive {
    number: u32,
    handle: HANDLE,
}

impl Drop for Drive {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

// ハンドルは計測のスレッドだけで使う
unsafe impl Send for Drive {}

/// ドライブ番号ごとの (読み取り, 書き込み) の累計
type Totals = HashMap<u32, (u64, u64)>;

#[derive(Default)]
pub struct Disk {
    drives: Vec<Drive>,
    scanned: Option<Instant>,
    rate: Rate<Totals>,
}

fn open(number: u32) -> Option<Drive> {
    let path: Vec<u16> = format!(r"\\.\PhysicalDrive{number}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    (handle != INVALID_HANDLE_VALUE).then_some(Drive { number, handle })
}

fn query(drive: &Drive) -> Option<(u64, u64)> {
    let mut perf: DISK_PERFORMANCE = unsafe { std::mem::zeroed() };
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            drive.handle,
            IOCTL_DISK_PERFORMANCE,
            std::ptr::null(),
            0,
            (&mut perf as *mut DISK_PERFORMANCE).cast(),
            std::mem::size_of::<DISK_PERFORMANCE>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    } != 0;
    ok.then_some((
        perf.BytesRead.max(0) as u64,
        perf.BytesWritten.max(0) as u64,
    ))
}

impl Disk {
    /// (読み取り, 書き込み) を B/s で返す。
    pub fn sample(&mut self) -> Option<(f64, f64)> {
        if self.scanned.is_none_or(|t| t.elapsed() >= RESCAN) {
            self.drives = (0..MAX_DRIVES).filter_map(open).collect();
            self.scanned = Some(Instant::now());
        }
        let now: Totals = self
            .drives
            .iter()
            .filter_map(|d| query(d).map(|v| (d.number, v)))
            .collect();
        if now.is_empty() {
            return None;
        }
        self.rate
            .update(now, |prev, now| Some(keyed_delta(prev, now)))
    }
}
