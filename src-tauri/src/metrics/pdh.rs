//! PDH（Windows の性能カウンター）の薄い包み。
//!
//! カウンターは英語名で足す。日本語版の Windows でも同じ名前で引けるため。

use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

const ERROR_SUCCESS: u32 = 0;

pub struct Query(PDH_HQUERY);

/// クエリに足したカウンター。クエリより長くは使わない。
#[derive(Clone, Copy)]
pub struct Counter(PDH_HCOUNTER);

// ハンドルは計測のスレッドだけで使う
unsafe impl Send for Query {}
unsafe impl Send for Counter {}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Query {
    pub fn open() -> Option<Self> {
        let mut query: PDH_HQUERY = std::ptr::null_mut();
        let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
        (status == ERROR_SUCCESS).then_some(Self(query))
    }

    pub fn add(&self, path: &str) -> Option<Counter> {
        let path = wide(path);
        let mut counter: PDH_HCOUNTER = std::ptr::null_mut();
        let status = unsafe { PdhAddEnglishCounterW(self.0, path.as_ptr(), 0, &mut counter) };
        if status != ERROR_SUCCESS {
            eprintln!("性能カウンターを足せませんでした（{status:#x}）");
            return None;
        }
        Some(Counter(counter))
    }

    /// 値を取り直す。毎秒の量は、前回の呼び出しとの差から求まる。
    pub fn collect(&self) -> bool {
        unsafe { PdhCollectQueryData(self.0) == ERROR_SUCCESS }
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.0);
        }
    }
}

impl Counter {
    /// インスタンス名に `*` を使ったカウンターの、インスタンスごとの値。
    pub fn values(&self) -> Vec<(String, f64)> {
        let mut size = 0u32;
        let mut count = 0u32;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                self.0,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if status != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // 名前の文字列も同じ領域に入るので、バイト数で取る。並びのため u64 で確保する
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let items = buf.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.0, PDH_FMT_DOUBLE, &mut size, &mut count, items)
        };
        if status != ERROR_SUCCESS {
            return Vec::new();
        }
        let items = unsafe { std::slice::from_raw_parts(items, count as usize) };
        items
            .iter()
            .filter(|item| item.FmtValue.CStatus == PDH_CSTATUS_VALID_DATA)
            .map(|item| {
                let name = unsafe { read_wide(item.szName) };
                (name, unsafe { item.FmtValue.Anonymous.doubleValue })
            })
            .collect()
    }
}

/// NUL で終わる UTF-16 の文字列を読む。
unsafe fn read_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}
