// c01_e01_variables.rs
//
// CONCEPT: `let`, immutability by default, `mut`.
// GUIDE:   chapters/01-basics/guide.md  (section "Variables and mutability")
// DATABROKER: databroker/src/broker.rs — `update_entries` builds its result
//             in `let mut errors = Vec::new();` — `mut` because it grows.
// EXPECT:  compile error E0384 (cannot assign twice to immutable variable).
//
// In Rust every variable is immutable unless you say `mut`. The databroker
// relies on this everywhere: a value you did not mark `mut` can never change
// underneath you, which makes code much easier to reason about.
//
// YOUR TASK
//   Make this file compile and the tests pass. Change only the code in
//   `count_updates` — never edit the tests.
//
// When the tests pass, delete the `// I AM NOT DONE` line below and run
// `cargo rl` to move on.

// I AM NOT DONE

/// Counts how many of the given speed readings (km/h) are above `limit`.
fn count_updates(readings: &[u32], limit: u32) -> u32 {
    let count = 0;
    for reading in readings {
        if *reading > limit {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_readings_above_limit() {
        assert_eq!(count_updates(&[10, 60, 120, 130], 100), 2);
    }

    #[test]
    fn empty_input_is_zero() {
        assert_eq!(count_updates(&[], 100), 0);
    }
}
