// c18_e07_review_macro_hygiene.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs typical
//          of chapter 18 (a macro that evaluates one of its arguments twice
//          instead of once, a repetition body that reuses the wrong
//          metavariable, and a recursive counting macro with a wrong base
//          case).
// GUIDE:   chapters/18-macros/guide.md  (section "Repetition")
// DATABROKER: databroker/src/main.rs — `concat!(...)` builds the `--version`
//             text at compile time; hand-written macros like these are used
//             the same way, so a subtly wrong expansion is silent until the
//             values it produces are wrong at runtime.
// EXPECT:  tests fail: a counter is incremented more times than expected,
//          a joined path repeats its first segment, and counting zero items
//          reports one.
//
// Three small macros used to build signal paths and pick between readings.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any macro's call syntax
//   (how it's invoked in the tests).

// I AM NOT DONE

use std::cell::Cell;

/// Counts the token trees passed to it (used to pre-size buffers at
/// compile time, the same trick `count!` uses elsewhere in this chapter).
macro_rules! count_tt {
    () => {
        1usize
    };
    ($head:tt $(, $tail:tt)*) => {
        1usize + count_tt!($($tail),*)
    };
}

/// Picks the smaller of two readings.
macro_rules! smaller_of {
    ($a:expr, $b:expr) => {
        if $a < $b {
            $a
        } else {
            $b
        }
    };
}

/// Joins a first VSS branch with any number of child segments into one
/// dotted path, e.g. `join_path!("Vehicle", "Cabin", "Seat")` ->
/// `"Vehicle.Cabin.Seat"`.
macro_rules! join_path {
    ($first:expr $(, $rest:expr)*) => {{
        let mut path = String::from($first);
        $(
            path.push('.');
            path.push_str($first);
        )*
        path
    }};
}

/// Reads a sensor value, recording that a read happened.
fn read_reading(reads: &Cell<u32>, value: i32) -> i32 {
    reads.set(reads.get() + 1);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_zero_items_is_zero() {
        assert_eq!(count_tt!(), 0);
    }

    #[test]
    fn counting_several_items() {
        assert_eq!(count_tt!(a, b, c), 3);
    }

    #[test]
    fn smaller_of_reads_each_side_exactly_once() {
        let reads = Cell::new(0);
        let smaller = smaller_of!(read_reading(&reads, 3), read_reading(&reads, 5));
        assert_eq!(smaller, 3);
        assert_eq!(reads.get(), 2, "each side should be evaluated exactly once");
    }

    #[test]
    fn join_path_uses_every_segment() {
        assert_eq!(join_path!("Vehicle", "Cabin", "Seat"), "Vehicle.Cabin.Seat");
    }
}
