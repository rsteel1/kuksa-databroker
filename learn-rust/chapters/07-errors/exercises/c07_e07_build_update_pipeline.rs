// c07_e07_build_update_pipeline.rs
//
// CONCEPT: writing a parsing pipeline from a spec: a `thiserror`-derived
//          error enum, `?` for fail-fast, and collecting errors instead
//          of propagating them.
// GUIDE:   chapters/07-errors/guide.md  (section "thiserror: the same, derived")
// DATABROKER: databroker/src/vss.rs — parses a VSS JSON tree and reports
//             precise errors per field; a provider's update stream is
//             parsed line by line the same way here.
// EXPECT:  tests fail: todo!() panics (and the error messages don't
//          exist yet until you derive them).
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. `ParseError`'s shape is fixed (the tests match on it
// directly), but you must derive `thiserror::Error` on it — with
// `#[error(...)]` attributes matching the doc comment on each variant —
// and implement the three functions.
//
// YOUR TASK
//   1. Add the right derives/attributes to `ParseError` so its `Display`
//      messages match the doc comments exactly.
//   2. Implement `parse_line`, `parse_all` and `parse_all_lenient`.
//   Do not change `ParseError`'s variants or any function's signature,
//   and do not edit the tests.

// I AM NOT DONE

// TODO: derive Debug, PartialEq and thiserror::Error, with an
// #[error("...")] attribute per variant matching its doc comment.
#[derive(Debug, PartialEq)]
enum ParseError {
    /// "empty line"
    Empty,
    /// "missing '=' in {0:?}" (the whole offending line)
    MissingEquals(String),
    /// "bad number {value:?} in {line:?}"
    BadNumber { line: String, value: String },
}

/// Parses one `"path=value"` line (leading/trailing whitespace ignored).
fn parse_line(line: &str) -> Result<(String, f64), ParseError> {
    todo!()
}

/// Parses every non-blank line of `input`, stopping at the first error.
fn parse_all(input: &str) -> Result<Vec<(String, f64)>, ParseError> {
    todo!()
}

/// Parses every non-blank line of `input`, never stopping: returns the
/// successfully parsed readings and every error encountered, in the
/// order the lines appeared.
fn parse_all_lenient(input: &str) -> (Vec<(String, f64)>, Vec<ParseError>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        assert_eq!(ParseError::Empty.to_string(), "empty line");
        assert_eq!(
            ParseError::MissingEquals("nope".to_string()).to_string(),
            "missing '=' in \"nope\""
        );
        assert_eq!(
            ParseError::BadNumber {
                line: "x=y".to_string(),
                value: "y".to_string()
            }
            .to_string(),
            "bad number \"y\" in \"x=y\""
        );
    }

    #[test]
    fn parses_a_good_line() {
        assert_eq!(
            parse_line("Vehicle.Speed=42.5"),
            Ok(("Vehicle.Speed".to_string(), 42.5))
        );
    }

    #[test]
    fn rejects_empty_line() {
        assert_eq!(parse_line("   "), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_missing_equals() {
        assert_eq!(
            parse_line("nope"),
            Err(ParseError::MissingEquals("nope".to_string()))
        );
    }

    #[test]
    fn rejects_bad_number() {
        assert_eq!(
            parse_line("Vehicle.Speed=abc"),
            Err(ParseError::BadNumber {
                line: "Vehicle.Speed=abc".to_string(),
                value: "abc".to_string()
            })
        );
    }

    #[test]
    fn parse_all_stops_at_first_error() {
        let input = "Vehicle.Speed=1\nbad-line\nVehicle.RPM=2";
        assert_eq!(
            parse_all(input),
            Err(ParseError::MissingEquals("bad-line".to_string()))
        );
    }

    #[test]
    fn parse_all_skips_blank_lines() {
        let input = "Vehicle.Speed=1\n\nVehicle.RPM=2\n";
        assert_eq!(
            parse_all(input),
            Ok(vec![
                ("Vehicle.Speed".to_string(), 1.0),
                ("Vehicle.RPM".to_string(), 2.0)
            ])
        );
    }

    #[test]
    fn parse_all_lenient_collects_everything() {
        let input = "Vehicle.Speed=1\nbad-line\nVehicle.RPM=oops";
        let (ok, errs) = parse_all_lenient(input);
        assert_eq!(ok, vec![("Vehicle.Speed".to_string(), 1.0)]);
        assert_eq!(errs.len(), 2);
    }
}
