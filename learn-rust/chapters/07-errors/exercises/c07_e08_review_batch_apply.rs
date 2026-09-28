// c07_e08_review_batch_apply.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 07 (a swallowed error that
//          should have aborted the batch, an off-by-one line number, an
//          error message that loses the real cause).
// GUIDE:   chapters/07-errors/guide.md  (section "Designing an error type")
// DATABROKER: databroker/src/vss.rs — a bad field aborts parsing the
//             whole VSS tree with a precise error rather than silently
//             skipping it; swallowing the error here (bug 2) would let
//             corrupt input pass as success.
// EXPECT:  tests fail: an invalid line doesn't abort the batch, the
//          reported line number is off by one, and an error message
//          doesn't mention the real line or entry.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

use std::collections::HashMap;

#[derive(Debug, PartialEq, thiserror::Error)]
enum ApplyError {
    #[error("line {line}: invalid entry {entry:?}")]
    Invalid { line: usize, entry: String },
}

/// Parses one line as `"path=value"`. `line_number` is the input's
/// 1-based line number, used only for the error message.
fn parse_entry(line_number: usize, raw: &str) -> Result<(String, f64), ApplyError> {
    let entry = raw.trim();
    let (path, value) = entry.split_once('=').ok_or_else(|| ApplyError::Invalid {
        line: line_number,
        entry: entry.to_string(),
    })?;
    let value: f64 = value.trim().parse().map_err(|_| ApplyError::Invalid {
        line: line_number,
        entry: entry.to_string(),
    })?;
    Ok((path.to_string(), value))
}

/// Applies every non-blank line of `input` as an update to `store`.
/// Returns the number of lines successfully applied. The FIRST invalid
/// line must abort the whole batch with an error — nothing after it
/// should be silently skipped.
fn apply_batch(store: &mut HashMap<String, f64>, input: &str) -> Result<usize, ApplyError> {
    let mut applied = 0;
    for (i, raw) in input.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        if let Ok((path, value)) = parse_entry(i, raw) {
            store.insert(path, value);
            applied += 1;
        }
    }
    Ok(applied)
}

/// A message for the first error in `results`, quoting its actual line
/// and entry — or `None` if every result is `Ok`.
fn first_error_message(results: &[Result<(String, f64), ApplyError>]) -> Option<String> {
    for r in results {
        if r.is_err() {
            return Some("invalid entry".to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entry_reports_one_based_line_numbers() {
        assert_eq!(
            parse_entry(1, "Vehicle.Speed=42.5"),
            Ok(("Vehicle.Speed".to_string(), 42.5))
        );
        assert_eq!(
            parse_entry(3, "bad-line"),
            Err(ApplyError::Invalid {
                line: 3,
                entry: "bad-line".to_string()
            })
        );
    }

    #[test]
    fn apply_batch_aborts_on_first_invalid_line() {
        let mut store = HashMap::new();
        let input = "Vehicle.Speed=1\nbad-line\nVehicle.RPM=2";
        let result = apply_batch(&mut store, input);
        assert_eq!(
            result,
            Err(ApplyError::Invalid {
                line: 2,
                entry: "bad-line".to_string()
            })
        );
        // Nothing after the bad line should have been applied either.
        assert_eq!(store.get("Vehicle.RPM"), None);
    }

    #[test]
    fn apply_batch_counts_valid_lines() {
        let mut store = HashMap::new();
        let input = "Vehicle.Speed=1\nVehicle.RPM=2\n";
        assert_eq!(apply_batch(&mut store, input), Ok(2));
    }

    #[test]
    fn first_error_message_quotes_the_real_entry() {
        let results = vec![
            Ok(("Vehicle.Speed".to_string(), 1.0)),
            Err(ApplyError::Invalid {
                line: 5,
                entry: "oops".to_string(),
            }),
        ];
        let message = first_error_message(&results).expect("there is an error");
        assert!(
            message.contains('5'),
            "message should mention the line: {message}"
        );
        assert!(
            message.contains("oops"),
            "message should mention the entry: {message}"
        );
    }
}
