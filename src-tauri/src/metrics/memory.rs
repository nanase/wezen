//! メモリの使用量。使用量は「全体 − 利用可能」とする。

use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// (使用量, 全体) をバイトで返す。
pub fn sample() -> Option<(f64, f64)> {
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    let total = status.ullTotalPhys as f64;
    let used = status.ullTotalPhys.saturating_sub(status.ullAvailPhys) as f64;
    Some((used, total))
}
