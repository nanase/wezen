//! ネットワークの送受信量。`GetIfTable2` の累計を前回と比べる。
//!
//! 物理アダプターだけを数える。VPN などの仮想アダプターを足すと、同じ通信を 2 回数えるため。
//! フィルター層（同じアダプターに重ねて見える層）とループバックも除く。

use super::rate::{keyed_delta, Rate};
use std::collections::HashMap;
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
    rate: Rate<Totals>,
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

impl Net {
    /// (受信, 送信) を B/s で返す。
    pub fn sample(&mut self) -> Option<(f64, f64)> {
        self.rate
            .update(read()?, |prev, now| Some(keyed_delta(prev, now)))
    }
}
