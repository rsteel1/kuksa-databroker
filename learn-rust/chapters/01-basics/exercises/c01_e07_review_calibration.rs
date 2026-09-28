// c01_e07_review_calibration.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 01 (a truncating cast, a
//          shadowed accumulator, an off-by-one loop bound).
// GUIDE:   chapters/01-basics/guide.md  (section "Scalar types and conversions")
// DATABROKER: databroker/src/types.rs — raw values are cast before being
//             stored as a `Datapoint`; a wrong cast or a dropped
//             accumulator here would silently corrupt a signal.
// EXPECT:  tests fail: three assertions give wrong numbers (no panics).
//
// A provider reports raw millivolt readings from a temperature sensor.
// `calibrate` converts millivolts to tenths of a degree Celsius;
// `running_total` sums a window of readings; `first_n_average` averages
// the first `n` readings of a window.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

/// Converts a raw millivolt reading to tenths of a degree Celsius:
/// `(mv - offset_mv) / 10`.
fn calibrate(mv: i32, offset_mv: i32) -> i32 {
    // BUG 1: Narrowing conversion
    let raw = mv - offset_mv;
    raw / 10
}

/// Sums every reading in `window`.
fn running_total(window: &[i32]) -> i32 {
    let mut total = 0;
    for i in 0..window.len() {
        // BUG 2: Shadowing total
        total += window[i];
    }
    total
}

/// Averages the first `n` readings of `window` (`n <= window.len()`).
fn first_n_average(window: &[i32], n: usize) -> f64 {
    let mut sum = 0i64;
    for i in 0..n {
        sum += window[i] as i64;
    }
    sum as f64 / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrate_large_reading() {
        // A reading far above the i16 range must not wrap around.
        assert_eq!(calibrate(50_000, 0), 5000);
    }

    #[test]
    fn calibrate_small_reading() {
        assert_eq!(calibrate(235, 15), 22);
    }

    #[test]
    fn running_total_sums_everything() {
        assert_eq!(running_total(&[1, 2, 3, 4]), 10);
    }

    #[test]
    fn first_n_average_uses_all_n() {
        assert_eq!(first_n_average(&[10, 20, 30, 40], 4), 25.0);
    }
}
