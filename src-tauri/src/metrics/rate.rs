//! 累計の値を前回と比べて、1 秒あたりの量にする。I/O、Disk、Network で使う。

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;

/// 前回読んだ累計と、その時刻。
pub struct Rate<T> {
    last: Option<(Instant, T)>,
}

impl<T> Default for Rate<T> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T> Rate<T> {
    /// 今回の累計 `now` を覚え、前回からの増分 `delta` を経過秒数で割る。
    /// 初回と、増分が求まらないとき（累計が戻ったなど）は None。
    pub fn update(
        &mut self,
        now: T,
        delta: impl FnOnce(&T, &T) -> Option<(u64, u64)>,
    ) -> Option<(f64, f64)> {
        let at = Instant::now();
        let rate = self.last.take().and_then(|(prev_at, prev)| {
            let secs = at.duration_since(prev_at).as_secs_f64();
            if secs <= 0.0 {
                return None;
            }
            let (a, b) = delta(&prev, &now)?;
            Some((a as f64 / secs, b as f64 / secs))
        });
        self.last = Some((at, now));
        rate
    }
}

/// 機器ごとの累計の、前回と今回の両方にある機器だけで、増えた量を足す。累計が減った機器は数えない。
pub fn keyed_delta<K: Eq + Hash>(
    prev: &HashMap<K, (u64, u64)>,
    now: &HashMap<K, (u64, u64)>,
) -> (u64, u64) {
    now.iter()
        .fold((0, 0), |(a, b), (key, (na, nb))| match prev.get(key) {
            Some((pa, pb)) => (
                a + na.checked_sub(*pa).unwrap_or(0),
                b + nb.checked_sub(*pb).unwrap_or(0),
            ),
            None => (a, b),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_devices_seen_twice_are_counted() {
        let prev = HashMap::from([(1, (100, 50)), (2, (1_000, 1_000))]);
        let now = HashMap::from([(1, (160, 80)), (2, (10, 1_200)), (3, (9_999, 9_999))]);
        // 2 の 1 つ目は累計が戻ったので数えない。3 は今回から現れたので数えない
        assert_eq!(keyed_delta(&prev, &now), (60, 230));
    }

    #[test]
    fn first_update_has_no_rate() {
        let mut rate = Rate::default();
        assert_eq!(rate.update(10u64, |_, _| Some((1, 1))), None);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let (a, _) = rate
            .update(20u64, |prev, now| Some((now - prev, 0)))
            .unwrap();
        assert!(a > 0.0);
    }
}
