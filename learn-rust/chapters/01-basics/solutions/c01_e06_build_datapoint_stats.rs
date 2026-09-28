// c01_e06_build_datapoint_stats.rs
//
// CONCEPT: writing several small functions from a spec: casts, control
//          flow and integer-overflow safety, on raw sensor readings.
// GUIDE:   chapters/01-basics/guide.md  (section "Scalar types and conversions")
// DATABROKER: databroker/src/types.rs — the Int8/Uint8 range checks that
//             validate a raw value before it becomes a `Datapoint`.
// EXPECT:  tests fail: todo!() panics.
//
// A provider on the vehicle bus reports raw integer ticks that must be
// scaled and range-checked before they become a signal value. This
// exercise has no missing piece for you to fix — the tests below ARE the
// spec. Implement each function so every test passes, using only what
// chapter 01 covers (variables, casts, functions, control flow, arrays
// and slices).
//
// YOUR TASK
//   Implement `clamp`, `scale_to_percent`, `count_out_of_range` and
//   `average` below.
//   Do not edit the tests.

/// Clamps `value` into `min..=max`.
fn clamp(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

/// Scales a raw reading in `0..=raw_max` to a percentage in `0..=100`.
/// The multiply must be done in a type wide enough that `raw * 100` can't
/// overflow, and the result must be clamped before the final cast to `u8`.
/// A non-positive `raw_max` is treated as "no signal": returns 0.
fn scale_to_percent(raw: i32, raw_max: i32) -> u8 {
    if raw_max <= 0 {
        return 0;
    }
    // WHY: widen to i64 before multiplying so `raw * 100` can't overflow.
    let scaled = (i64::from(raw) * 100) / i64::from(raw_max);
    scaled.clamp(0, 100) as u8
}

/// Counts how many readings fall outside `min..=max`.
fn count_out_of_range(readings: &[i32], min: i32, max: i32) -> usize {
    let mut count = 0;
    for &r in readings {
        if !(min..=max).contains(&r) {
            count += 1;
        }
    }
    count
}

/// The arithmetic mean of `readings` as an `f64`. Returns `0.0` for an
/// empty slice.
fn average(readings: &[i32]) -> f64 {
    if readings.is_empty() {
        return 0.0;
    }
    let mut sum: i64 = 0;
    for &r in readings {
        sum += i64::from(r);
    }
    sum as f64 / readings.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamping() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
    }

    #[test]
    fn scaling() {
        assert_eq!(scale_to_percent(0, 1000), 0);
        assert_eq!(scale_to_percent(1000, 1000), 100);
        assert_eq!(scale_to_percent(500, 1000), 50);
        // Above raw_max must clamp to 100, not wrap or panic.
        assert_eq!(scale_to_percent(2000, 1000), 100);
        // The multiply must not overflow i32 (30_000_000 * 100 > i32::MAX).
        assert_eq!(scale_to_percent(30_000_000, 100), 100);
        assert_eq!(scale_to_percent(5, 0), 0);
    }

    #[test]
    fn out_of_range_count() {
        let readings = [-10, 0, 5, 12, 20, 7];
        assert_eq!(count_out_of_range(&readings, 0, 10), 3);
        assert_eq!(count_out_of_range(&readings, -100, 100), 0);
    }

    #[test]
    fn averages() {
        assert_eq!(average(&[2, 4, 6]), 4.0);
        assert_eq!(average(&[]), 0.0);
    }
}
