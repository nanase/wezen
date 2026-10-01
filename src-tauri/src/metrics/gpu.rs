//! GPU の使用率とメモリ。
//!
//! - GPU の一覧と専用メモリの全体は DXGI から取る
//! - 使用率は `\GPU Engine(*)\Utilization Percentage` から、タスクマネージャーと同じく
//!   エンジンごとにプロセスの分を合計し、その最大を使う
//! - 専用メモリの使用量は `\GPU Adapter Memory(*)\Dedicated Usage` から取る

use serde::Serialize;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
};

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    /// LUID を性能カウンターのインスタンス名と同じ形にしたもの。設定に保存する
    pub id: String,
    pub name: String,
    /// 専用メモリの全体（B）
    pub memory: f64,
}

/// 性能カウンターのインスタンス名に入る LUID の形（小文字にそろえる）。
pub fn luid_id(high: i32, low: u32) -> String {
    format!("luid_0x{:08x}_0x{:08x}", high as u32, low)
}

/// ソフトウェアで描く仮のアダプター（Microsoft Basic Render Driver）を除いた GPU の一覧。
pub fn adapters() -> Vec<Adapter> {
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else {
            break;
        };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let len = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        let id = luid_id(desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart);
        // 同じ GPU が出力ごとに重ねて見えることがあるので、LUID で 1 つにする
        if out.iter().any(|a: &Adapter| a.id == id) {
            continue;
        }
        out.push(Adapter {
            id,
            name: String::from_utf16_lossy(&desc.Description[..len])
                .trim()
                .to_owned(),
            memory: desc.DedicatedVideoMemory as f64,
        });
    }
    out
}

/// 設定で選んだ GPU。選んでいない、または見つからなければ、専用メモリが最も多いもの。
pub fn pick<'a>(adapters: &'a [Adapter], chosen: Option<&str>) -> Option<&'a Adapter> {
    chosen
        .and_then(|id| adapters.iter().find(|a| a.id == id))
        .or_else(|| adapters.iter().max_by(|a, b| a.memory.total_cmp(&b.memory)))
}

/// インスタンス名のうち、LUID から始まる部分を小文字で返す。
fn from_luid(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let at = lower.find("luid_")?;
    Some(lower[at..].to_owned())
}

/// 使用率（%）。エンジン（`luid_…_phys_0_eng_0_engtype_3D` まで）ごとにプロセスの分を合計し、その最大。
pub fn usage(values: &[(String, f64)], adapter: &str) -> f64 {
    let mut engines: Vec<(String, f64)> = Vec::new();
    for (name, value) in values {
        let Some(engine) = from_luid(name) else {
            continue;
        };
        if !engine.starts_with(adapter) {
            continue;
        }
        match engines.iter_mut().find(|(e, _)| *e == engine) {
            Some((_, sum)) => *sum += value,
            None => engines.push((engine, *value)),
        }
    }
    engines
        .iter()
        .map(|(_, v)| *v)
        .fold(0.0, f64::max)
        .clamp(0.0, 100.0)
}

/// 専用メモリの使用量（B）。同じ GPU の物理アダプター（phys_0, phys_1…）を合計する。
pub fn dedicated(values: &[(String, f64)], adapter: &str) -> f64 {
    values
        .iter()
        .filter(|(name, _)| from_luid(name).is_some_and(|n| n.starts_with(adapter)))
        .map(|(_, v)| v)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPU: &str = "luid_0x00000000_0x0000d1f3";

    #[test]
    fn luid_matches_counter_instance_names() {
        assert_eq!(luid_id(0, 0xD1F3), GPU);
        assert_eq!(luid_id(-1, 1), "luid_0xffffffff_0x00000001");
    }

    #[test]
    fn usage_sums_processes_per_engine_and_takes_the_busiest() {
        let values = vec![
            (
                "pid_10_luid_0x00000000_0x0000D1F3_phys_0_eng_0_engtype_3D".into(),
                20.0,
            ),
            (
                "pid_20_luid_0x00000000_0x0000D1F3_phys_0_eng_0_engtype_3D".into(),
                15.0,
            ),
            (
                "pid_20_luid_0x00000000_0x0000D1F3_phys_0_eng_3_engtype_VideoDecode".into(),
                30.0,
            ),
            // ほかの GPU は数えない
            (
                "pid_30_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D".into(),
                90.0,
            ),
        ];
        assert_eq!(usage(&values, GPU), 35.0);
        assert_eq!(usage(&[], GPU), 0.0);
    }

    #[test]
    fn dedicated_memory_sums_physical_adapters() {
        let values = vec![
            ("luid_0x00000000_0x0000D1F3_phys_0".into(), 1_000.0),
            ("luid_0x00000000_0x0000D1F3_phys_1".into(), 500.0),
            ("luid_0x00000000_0x0000AAAA_phys_0".into(), 7_000.0),
        ];
        assert_eq!(dedicated(&values, GPU), 1_500.0);
    }

    #[test]
    fn picks_the_chosen_gpu_or_the_one_with_most_memory() {
        let list = vec![
            Adapter {
                id: "a".into(),
                name: "内蔵".into(),
                memory: 128.0,
            },
            Adapter {
                id: "b".into(),
                name: "外付け".into(),
                memory: 8_000.0,
            },
        ];
        assert_eq!(pick(&list, None).map(|a| a.id.as_str()), Some("b"));
        assert_eq!(pick(&list, Some("a")).map(|a| a.id.as_str()), Some("a"));
        assert_eq!(pick(&list, Some("gone")).map(|a| a.id.as_str()), Some("b"));
    }
}
