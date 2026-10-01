//! メモリの使用量。物理メモリとページファイル（スワップ領域）を合わせた、コミット済みの量を出す。
//!
//! タスクマネージャーの「コミット済み」、Process Explorer の Commit Charge と同じ値になる。

use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// (使用量, 上限) をバイトで返す。上限は物理メモリとページファイルの合計。
pub fn sample() -> Option<(f64, f64)> {
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    // 名前は PageFile だが、中身はコミットの上限と、その残り
    let limit = status.ullTotalPageFile;
    let used = limit.saturating_sub(status.ullAvailPageFile);
    Some((used as f64, limit as f64))
}
