// c17_e01_combinators.rs
//
// CONCEPT: the Option/Result combinators — map, and_then, ok_or_else,
//          unwrap_or_else, map_or, transpose, flatten, find_map — and why
//          `ok_or_else` (lazy) beats `ok_or` (eager) when the error is costly.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Combinators: small tools for Option and Result")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs:673 — `.ok_or(tonic::Status::not_found(format!(..)))`
//             builds (and allocates) the error message even when the path
//             *is* found; `.ok_or_else(|| ..)` only builds it on failure.
// EXPECT:  tests fail: `todo!()` panics
//
// Combinators let you transform the value inside an Option/Result without
// unpacking it with `match`. A chain of them reads like a sentence:
// "strip the unit, then parse, then convert". Each function below has a
// one-expression solution; nested `match`es also pass the tests, but try
// to write each body as a single chain.
//
// YOUR TASK
//   Replace every `todo!()`. Do not edit the tests.

// I AM NOT DONE

use std::cell::Cell;
use std::collections::HashMap;
use std::num::ParseIntError;

/// "12.5 m/s" -> Some(45.0) (km/h). Anything without the " m/s" suffix, or
/// whose number doesn't parse, is None.
/// Use `strip_suffix`, `and_then`, `.ok()` and `map`.
fn speed_kmh(raw: &str) -> Option<f64> {
    todo!()
}

/// The optional `interval_ms` query parameter: absent is fine (Ok(None)),
/// present-and-valid is Ok(Some(n)), present-and-invalid is an error.
/// Use `map` + `transpose` (Option<Result<T, E>> -> Result<Option<T>, E>).
fn parse_interval(param: Option<&str>) -> Result<Option<u32>, ParseIntError> {
    todo!()
}

/// Looks up a signal id by path. On a miss, the error message is
/// "unknown signal: <path>" and `error_builds` is incremented, so the tests
/// can check the message is only built when it's needed.
/// Use `get`, `copied` and `ok_or_else`.
fn lookup_id(
    ids: &HashMap<String, i32>,
    path: &str,
    error_builds: &Cell<u32>,
) -> Result<i32, String> {
    let build_error = || {
        error_builds.set(error_builds.get() + 1);
        format!("unknown signal: {path}")
    };
    let _ = build_error;
    todo!()
}

/// The first reading that parses as a u32, skipping garbage.
/// Use `iter().find_map(..)`.
fn first_valid_reading(readings: &[&str]) -> Option<u32> {
    todo!()
}

/// An actuator target is `Option<Option<i32>>`: outer None = "not an
/// actuator", Some(None) = "no target set", Some(Some(v)) = "target v".
/// Return the target if there is one. Use `flatten`.
fn target_value(actuator_target: Option<Option<i32>>) -> Option<i32> {
    todo!()
}

/// Human-readable label: "<value> <unit>" if the unit is known, otherwise
/// just "<value>". Use `map_or_else` (or `map` + `unwrap_or_else`).
fn label(value: i64, unit: Option<&str>) -> String {
    todo!()
}

/// Parses "min..max" (e.g. "0..250") into (min, max). Any failure (missing
/// "..", bad numbers, or min > max) gives a descriptive error.
/// Use `split_once` + `ok_or_else`, the `?` operator, and `map_err`.
fn parse_range(spec: &str) -> Result<(i64, i64), String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed() {
        assert_eq!(speed_kmh("12.5 m/s"), Some(45.0));
        assert_eq!(speed_kmh("0 m/s"), Some(0.0));
        assert_eq!(speed_kmh("12.5 km/h"), None);
        assert_eq!(speed_kmh("fast m/s"), None);
    }

    #[test]
    fn interval() {
        assert_eq!(parse_interval(None), Ok(None));
        assert_eq!(parse_interval(Some("100")), Ok(Some(100)));
        assert!(parse_interval(Some("-1")).is_err());
        assert!(parse_interval(Some("soon")).is_err());
    }

    #[test]
    fn lookup_builds_error_only_on_miss() {
        let ids = HashMap::from([("Vehicle.Speed".to_string(), 7)]);
        let builds = Cell::new(0);
        assert_eq!(lookup_id(&ids, "Vehicle.Speed", &builds), Ok(7));
        assert_eq!(
            builds.get(),
            0,
            "the error was built even though the lookup succeeded (ok_or vs ok_or_else)"
        );
        assert_eq!(
            lookup_id(&ids, "Vehicle.Nope", &builds),
            Err("unknown signal: Vehicle.Nope".to_string())
        );
        assert_eq!(builds.get(), 1);
    }

    #[test]
    fn first_valid() {
        assert_eq!(first_valid_reading(&["", "x", "42", "43"]), Some(42));
        assert_eq!(first_valid_reading(&["x", "-1"]), None);
        assert_eq!(first_valid_reading(&[]), None);
    }

    #[test]
    fn target() {
        assert_eq!(target_value(None), None);
        assert_eq!(target_value(Some(None)), None);
        assert_eq!(target_value(Some(Some(3))), Some(3));
    }

    #[test]
    fn labels() {
        assert_eq!(label(120, Some("km/h")), "120 km/h");
        assert_eq!(label(-4, None), "-4");
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("0..250"), Ok((0, 250)));
        assert_eq!(parse_range("-40..85"), Ok((-40, 85)));
        assert!(parse_range("0-250")
            .unwrap_err()
            .contains("expected MIN..MAX"));
        assert!(parse_range("a..3").unwrap_err().contains("bad min"));
        assert!(parse_range("1..b").unwrap_err().contains("bad max"));
        assert!(parse_range("9..1").unwrap_err().contains("greater than"));
    }
}
