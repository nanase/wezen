//! ネットワークの送受信量。`GetIfTable2` の累計を前回と比べる。
//!
//! 物理アダプターだけを数える。VPN などの仮想アダプターを足すと、同じ通信を 2 回数えるため。
//! フィルター層（同じアダプターに重ねて見える層）とループバックも除く。

use std::collections::HashMap;
use std::time::Instant;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;

/// InterfaceAndOperStatusFlags の各ビット
const HARDWARE_INTERFACE: u8 = 1 << 0;
const FILTER_INTERFACE: u8 = 1 << 1;

/// アダプターの LUID ごとの (受信, 送信) の累計
type Totals = HashMap<u64, (u64, u64)>;

#[derive(Default)]
pub struct Net {
    last: Option<(Instant, Totals)>,
}

fn counted(row: &MIB_IF_ROW2) -> bool {
    let flags = row.InterfaceAndOperStatusFlags._bitfield;
    flags & HARDWARE_INTERFACE != 0
        && flags & FILTER_INTERFACE == 0
        && row.Type != IF_TYPE_SOFTWARE_LOOPBACK
        && row.OperStatus == IfOperStatusUp
}

fn read() -> Option<Totals> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != 0 || table.is_null() {
        return None;
    }
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let map = rows
        .iter()
        .filter(|row| counted(row))
        .map(|row| {
            (
                unsafe { row.InterfaceLuid.Value },
                (row.InOctets, row.OutOctets),
            )
        })
        .collect();
    unsafe { FreeMibTable(table.cast()) };
    Some(map)
}

/// 前回と今回の両方にあるアダプターだけで、増えた量を足す。累計が減ったアダプターは数えない。
fn delta(prev: &Totals, now: &Totals) -> (u64, u64) {
    now.iter()
        .fold((0, 0), |(r, s), (luid, (rx, tx))| match prev.get(luid) {
            Some((prx, ptx)) => (
                r + rx.checked_sub(*prx).unwrap_or(0),
                s + tx.checked_sub(*ptx).unwrap_or(0),
            ),
            None => (r, s),
        })
}

impl Net {
    /// (受信, 送信) を B/s で返す。
    pub fn sample(&mut self) -> Option<(f64, f64)> {
        let now = read()?;
        let at = Instant::now();
        let (prev_at, prev) = self.last.replace((at, now.clone()))?;
        let secs = at.duration_since(prev_at).as_secs_f64();
        if secs <= 0.0 {
            return None;
        }
        let (rx, tx) = delta(&prev, &now);
        Some((rx as f64 / secs, tx as f64 / secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_adapters_seen_twice_are_counted() {
        let prev = HashMap::from([(1, (100, 50)), (2, (1_000, 1_000))]);
        let now = HashMap::from([(1, (160, 80)), (2, (10, 1_200)), (3, (9_999, 9_999))]);
        // 2 の受信は累計が戻ったので数えない。3 は今回から現れたので数えない
        assert_eq!(delta(&prev, &now), (60, 230));
    }
}
