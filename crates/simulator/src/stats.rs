//! Win-rate statistics: Wilson 95% interval and the text forms the report prints.
//!
//! Numeric determinism: only `+ - * /` and `sqrt` on `f64` (exactly rounded by IEEE 754, so
//! identical on every platform) -- never `powi`, `ln` or `exp` (libm results vary).

/// Wilson 95% interval (`z = 1.96`) of `successes` out of `games`, as fractions in `[0, 1]`.
/// `None` when `games == 0` or `successes > games`.
pub fn wilson_95(successes: u64, games: u64) -> Option<(f64, f64)> {
    if games == 0 || successes > games {
        return None;
    }
    const Z: f64 = 1.96;
    let n = games as f64;
    let p = successes as f64 / n;
    let z2 = Z * Z;
    let denom = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denom;
    let half = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;
    // The interval is exact at the edges (nothing below 0 successes, nothing above `games`) but
    // the float result can be off by an ulp there (`-0.0`, `0.9999999999999999`): those bounds are
    // snapped to 0 and 1. Strictly inside, Wilson's bounds are strictly inside `(0, 1)`, so no
    // further clamp can ever change a value.
    let low = if successes == 0 { 0.0 } else { centre - half };
    let high = if successes == games {
        1.0
    } else {
        centre + half
    };
    Some((low, high))
}

/// `100 * successes / games`, `None` when `games == 0`.
pub fn percent(successes: u64, games: u64) -> Option<f64> {
    (games != 0).then(|| 100.0 * successes as f64 / games as f64)
}

/// `"51.2% [48.1-54.3]"` (share and its Wilson 95% interval, one decimal), or `"n/a"`.
pub fn fmt_share(successes: u64, games: u64) -> String {
    match (percent(successes, games), wilson_95(successes, games)) {
        (Some(share), Some((low, high))) => {
            format!("{share:.1}% [{:.1}-{:.1}]", 100.0 * low, 100.0 * high)
        }
        _ => "n/a".to_string(),
    }
}

/// `total / games` to one decimal, or `"n/a"` when `games == 0`.
pub fn fmt_mean(total: u64, games: u64) -> String {
    if games == 0 {
        return "n/a".to_string();
    }
    format!("{:.1}", total as f64 / games as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds_pct(k: u64, n: u64) -> (String, String) {
        let (lo, hi) = wilson_95(k, n).expect("valid input");
        (format!("{:.1}", 100.0 * lo), format!("{:.1}", 100.0 * hi))
    }

    #[test]
    fn reference_values_500_of_1000() {
        assert_eq!(fmt_share(500, 1000), "50.0% [46.9-53.1]");
    }

    #[test]
    fn reference_values_zero_successes() {
        assert_eq!(fmt_share(0, 10), "0.0% [0.0-27.8]");
        assert_eq!(bounds_pct(0, 10), ("0.0".into(), "27.8".into()));
    }

    #[test]
    fn reference_values_all_successes() {
        assert_eq!(fmt_share(10, 10), "100.0% [72.2-100.0]");
    }

    #[test]
    fn reference_values_tiny_samples() {
        assert_eq!(fmt_share(1, 2), "50.0% [9.5-90.5]");
        assert_eq!(fmt_share(0, 1), "0.0% [0.0-79.3]");
        assert_eq!(fmt_share(1, 1), "100.0% [20.7-100.0]");
    }

    #[test]
    fn lower_bound_is_never_negative_zero() {
        for n in 1..=200 {
            let (lo, _) = wilson_95(0, n).unwrap();
            assert!(lo == 0.0 && lo.is_sign_positive(), "n = {n}: {lo:?}");
            assert!(!fmt_share(0, n).contains("-0.0"));
        }
    }

    #[test]
    fn interval_contains_the_proportion_and_stays_in_unit_range() {
        for n in 1..=60u64 {
            for k in 0..=n {
                let (lo, hi) = wilson_95(k, n).unwrap();
                let p = k as f64 / n as f64;
                assert!(lo <= p && p <= hi, "{k}/{n}: {lo} {p} {hi}");
                assert!((0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi));
            }
        }
    }

    #[test]
    fn interval_shrinks_as_games_grow_at_fixed_proportion() {
        let width = |k: u64, n: u64| {
            let (lo, hi) = wilson_95(k, n).unwrap();
            hi - lo
        };
        assert!(width(5, 10) > width(50, 100));
        assert!(width(50, 100) > width(500, 1000));
        assert!(width(500, 1000) > width(5000, 10000));
    }

    #[test]
    fn interval_is_mirrored_for_complementary_counts() {
        for n in 1..=60u64 {
            for k in 0..=n {
                let (lo, hi) = wilson_95(k, n).unwrap();
                let (lo2, hi2) = wilson_95(n - k, n).unwrap();
                assert!((lo - (1.0 - hi2)).abs() < 1e-12, "{k}/{n}");
                assert!((hi - (1.0 - lo2)).abs() < 1e-12, "{k}/{n}");
            }
        }
    }

    #[test]
    fn no_games_or_too_many_successes_has_no_interval() {
        assert_eq!(wilson_95(0, 0), None);
        assert_eq!(wilson_95(3, 2), None);
        assert_eq!(percent(0, 0), None);
        assert_eq!(fmt_share(0, 0), "n/a");
        assert_eq!(fmt_share(3, 2), "n/a");
        assert_eq!(fmt_mean(5, 0), "n/a");
    }

    #[test]
    fn percent_and_mean() {
        assert_eq!(percent(1, 4), Some(25.0));
        assert_eq!(fmt_mean(3910, 96), "40.7");
        assert_eq!(fmt_mean(0, 5), "0.0");
    }
}
