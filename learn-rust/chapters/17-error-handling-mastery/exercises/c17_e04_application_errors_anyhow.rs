// c17_e04_application_errors_anyhow.rs
//
// CONCEPT: in *application* code (binaries, top-level glue) you rarely match
//          on error variants. You want to add context and report the chain.
//          `anyhow::Error` holds any error plus a stack of context messages.
//          `.context()`, `.with_context(|| ..)`, `bail!`, `ensure!`,
//          `{:#}` / `{:?}` formatting, and `downcast_ref` when you *do*
//          need to branch on a specific underlying error.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Application errors with anyhow")
// DATABROKER: databroker/src/main.rs:153 `read_metadata_file` and main.rs:232
//             `fn main() -> Result<(), Box<dyn std::error::Error>>` do the same
//             job with `Box<dyn Error>`, but without the context messages: a
//             bad VSS file is reported by the serde message alone.
// EXPECT:  tests fail: `todo!()` panics
//
// Rule of thumb: libraries return typed errors (thiserror) so callers can
// react; applications wrap them in anyhow and add context on the way up so
// the *operator* can understand the failure.
//
// YOUR TASK
//   Implement `parse_assignments`, `load_config` and `is_missing_signal`.
//   Do not edit the tests.

// I AM NOT DONE

use anyhow::{bail, ensure, Context, Result};
use std::collections::HashMap;

/// A *library* error (typed, matchable) that the application will wrap.
#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("signal not found: {0}")]
    NotFound(String),
}

fn lookup(known: &HashMap<String, i64>, path: &str) -> Result<i64, LookupError> {
    known
        .get(path)
        .copied()
        .ok_or_else(|| LookupError::NotFound(path.to_string()))
}

/// Parses lines of the form `Path=integer`. Blank lines and lines starting
/// with '#' are skipped. Errors (each must mention the 1-based line number):
///   * no '='                  -> "line N: expected PATH=VALUE"  (use `with_context` or `bail!`)
///   * empty path              -> "line N: empty signal path"    (use `ensure!`)
///   * value doesn't parse     -> context "line N: invalid value for PATH" on top of the ParseIntError
///   * same path twice         -> "line N: duplicate assignment to PATH" (use `bail!`)
fn parse_assignments(text: &str) -> Result<Vec<(String, i64)>> {
    todo!()
}

/// Loads a config: parses it (adding the context "failed to parse config
/// <name>"), then checks every path exists in `known` via `lookup`,
/// adding the context "config <name> refers to an unknown signal".
fn load_config(name: &str, text: &str, known: &HashMap<String, i64>) -> Result<Vec<(String, i64)>> {
    todo!()
}

/// True if, anywhere in the chain, the error is a `LookupError::NotFound`.
/// (An application might turn exactly this case into "exit code 2".)
fn is_missing_signal(err: &anyhow::Error) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> HashMap<String, i64> {
        HashMap::from([
            ("Vehicle.Speed".to_string(), 0),
            ("Vehicle.Cabin.Temp".to_string(), 21),
        ])
    }

    #[test]
    fn parses_good_input() {
        let text = "# defaults\nVehicle.Speed = 10\n\nVehicle.Cabin.Temp=22\n";
        assert_eq!(
            parse_assignments(text).unwrap(),
            vec![
                ("Vehicle.Speed".to_string(), 10),
                ("Vehicle.Cabin.Temp".to_string(), 22)
            ]
        );
    }

    #[test]
    fn missing_equals() {
        let err = parse_assignments("Vehicle.Speed 10").unwrap_err();
        assert_eq!(err.to_string(), "line 1: expected PATH=VALUE");
    }

    #[test]
    fn empty_path() {
        let err = parse_assignments("# x\n =3").unwrap_err();
        assert_eq!(err.to_string(), "line 2: empty signal path");
    }

    #[test]
    fn bad_value_keeps_the_cause() {
        let err = parse_assignments("Vehicle.Speed=fast").unwrap_err();
        // `{}` shows only the outermost message ...
        assert_eq!(err.to_string(), "line 1: invalid value for Vehicle.Speed");
        // ... `{:#}` shows the whole chain on one line.
        assert_eq!(
            format!("{err:#}"),
            "line 1: invalid value for Vehicle.Speed: invalid digit found in string"
        );
        assert!(err.downcast_ref::<std::num::ParseIntError>().is_some());
    }

    #[test]
    fn duplicates() {
        let err = parse_assignments("A=1\nB=2\nA=3").unwrap_err();
        assert_eq!(err.to_string(), "line 3: duplicate assignment to A");
    }

    #[test]
    fn load_adds_context_layers() {
        let err = load_config("seat.conf", "Vehicle.Speed=x", &known()).unwrap_err();
        let chain: Vec<String> = err.chain().map(|e| e.to_string()).collect();
        assert_eq!(
            chain,
            vec![
                "failed to parse config seat.conf",
                "line 1: invalid value for Vehicle.Speed",
                "invalid digit found in string",
            ]
        );
        assert!(!is_missing_signal(&err));
    }

    #[test]
    fn unknown_signal_is_detectable_through_context() {
        let err = load_config("seat.conf", "Vehicle.Nope=1", &known()).unwrap_err();
        assert_eq!(
            format!("{err:#}"),
            "config seat.conf refers to an unknown signal: signal not found: Vehicle.Nope"
        );
        assert!(is_missing_signal(&err));
    }

    #[test]
    fn load_ok() {
        let got = load_config("ok.conf", "Vehicle.Speed=5", &known()).unwrap();
        assert_eq!(got, vec![("Vehicle.Speed".to_string(), 5)]);
    }
}
