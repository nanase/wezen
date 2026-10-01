//! CPU の使用率。`GetSystemTimes` を 2 回呼んだ差から求める（Process Explorer と同じ考え方）。

use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Threading::GetSystemTimes;

#[derive(Clone, Copy)]
struct Times {
    idle: u64,
    /// カーネルの時間。アイドルの時間を含む
    kernel: u64,
    user: u64,
}

#[derive(Default)]
pub struct Cpu {
    last: Option<Times>,
}

fn ticks(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

fn read() -> Option<Times> {
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut idle, mut kernel, mut user) = (zero, zero, zero);
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
    ok.then(|| Times {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    })
}

/// 2 回の読み取りの差から求めた使用率（%）。
fn usage(prev: Times, now: Times) -> Option<f64> {
    let total = (now.kernel + now.user).checked_sub(prev.kernel + prev.user)?;
    let idle = now.idle.checked_sub(prev.idle)?;
    if total == 0 {
        return None;
    }
    Some((total.saturating_sub(idle) as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
}

impl Cpu {
    pub fn sample(&mut self) -> Option<f64> {
        let now = read()?;
        let prev = self.last.replace(now)?;
        usage(prev, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_share_of_elapsed_time() {
        let prev = Times {
            idle: 1_000,
            kernel: 2_000,
            user: 1_000,
        };
        // 経過 400 のうちアイドル 300
        let now = Times {
            idle: 1_300,
            kernel: 2_350,
            user: 1_050,
        };
        assert_eq!(usage(prev, now), Some(25.0));
        assert_eq!(usage(prev, prev), None);
    }
}
