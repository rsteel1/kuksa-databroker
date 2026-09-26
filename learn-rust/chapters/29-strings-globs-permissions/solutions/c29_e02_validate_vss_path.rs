// c29_e02_validate_vss_path.rs
//
// CONCEPT: validating input two ways — iterator logic and a lazily-compiled regex.
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "Validating paths: iterators vs regex")
// DATABROKER: databroker/src/glob.rs — `REGEX_VALID_PATH` (lazy_static!) and `is_valid_path`
// EXPECT:  tests fail: `todo!()` panics
//
// Before registering or looking up a signal, the databroker checks that the
// path is well formed. A valid path is one or more non-empty segments joined
// by '.', where a segment contains no '.', no ':', no '*' and no whitespace.
//
// The databroker does this with a regex stored in a `lazy_static!`: compiling
// a regex is expensive, so it happens once, on first use. Modern std has the
// same thing built in: `std::sync::LazyLock` (Rust 1.80+). New code should
// prefer it over the `lazy_static` crate.
//
// YOUR TASK
//   1. `is_valid_path`: implement with `split('.')` + `all(...)` — no regex.
//   2. `VALID_PATH`: initialise the LazyLock with a regex implementing the
//      same rule, and implement `is_valid_path_regex` with it.
//      Tests check that both functions agree on every input.
//   Do not edit the tests.

use regex::Regex;
use std::sync::LazyLock;

fn is_valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment
            .chars()
            .any(|c| c == '.' || c == ':' || c == '*' || c.is_whitespace())
}

fn is_valid_path(path: &str) -> bool {
    // WHY: "" splits into one empty segment, so it's rejected too.
    path.split('.').all(is_valid_segment)
}

static VALID_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^.:*\s]+(?:\.[^.:*\s]+)*$").expect("static regex is valid"));

fn is_valid_path_regex(path: &str) -> bool {
    VALID_PATH.is_match(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &[&str] = &[
        "Vehicle",
        "Vehicle.Speed",
        "Vehicle.Cabin.Door.Row1.DriverSide.IsOpen",
        "Kuksa.Databroker.CargoVersion",
        "Vehicle.OBD.O2WR.Sensor1.Lambda",
    ];

    const INVALID: &[&str] = &[
        "",
        ".",
        "Vehicle.",
        ".Vehicle",
        "Vehicle..Speed",
        "Vehicle.*",
        "Vehicle.**.Speed",
        "Vehicle:Speed",
        "Vehicle Speed",
        "Vehicle.\tSpeed",
    ];

    #[test]
    fn iterator_version() {
        for p in VALID {
            assert!(is_valid_path(p), "{p:?} should be valid");
        }
        for p in INVALID {
            assert!(!is_valid_path(p), "{p:?} should be invalid");
        }
    }

    #[test]
    fn regex_version_agrees() {
        for p in VALID.iter().chain(INVALID) {
            assert_eq!(
                is_valid_path_regex(p),
                is_valid_path(p),
                "the two implementations disagree on {p:?}"
            );
        }
    }
}
