//! I/O の読み書き量。システム全体の累計を `NtQuerySystemInformation` で取り、前回と比べる。
//!
//! Process Explorer の I/O と同じ元データで、性能カウンターの `\System\File Read Bytes/sec` なども
//! ここから作られる。性能カウンターは登録が壊れていると引けないので、元データを直接読む。

use super::rate::Rate;
use windows_sys::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemPerformanceInformation,
};

/// SYSTEM_PERFORMANCE_INFORMATION の大きさより大きく取る（Windows の版で大きさが変わる）
const BUF_WORDS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Totals {
    read: u64,
    write: u64,
    other: u64,
}

#[derive(Default)]
pub struct Io {
    rate: Rate<Totals>,
}

fn read() -> Option<Totals> {
    let mut buf = [0u64; BUF_WORDS];
    let mut len = 0u32;
    let status = unsafe {
        NtQuerySystemInformation(
            SystemPerformanceInformation,
            buf.as_mut_ptr().cast(),
            (BUF_WORDS * 8) as u32,
            &mut len,
        )
    };
    if status < 0 {
        return None;
    }
    // 先頭は IdleProcessTime、続いて IoReadTransferCount、IoWriteTransferCount、IoOtherTransferCount。
    // この並びは NT のころから変わっていない
    Some(Totals {
        read: buf[1],
        write: buf[2],
        other: buf[3],
    })
}

/// (読み取り + その他, 書き込み) の増えた量。累計が戻っていれば None。
fn delta(prev: &Totals, now: &Totals) -> Option<(u64, u64)> {
    let read = now.read.checked_sub(prev.read)?;
    let other = now.other.checked_sub(prev.other)?;
    let write = now.write.checked_sub(prev.write)?;
    Some((read + other, write))
}

impl Io {
    /// (R+O, W) を B/s で返す。
    pub fn sample(&mut self) -> Option<(f64, f64)> {
        self.rate.update(read()?, delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_other_are_added() {
        let prev = Totals {
            read: 100,
            write: 50,
            other: 10,
        };
        let now = Totals {
            read: 400,
            write: 90,
            other: 30,
        };
        assert_eq!(delta(&prev, &now), Some((320, 40)));
        assert_eq!(delta(&now, &prev), None);
    }
}
