// c07_e01_result_basics.rs
//
// CONCEPT: Result<T, E> — success or failure as a value; matching on it.
// GUIDE:   chapters/07-errors/guide.md  (section "Result: errors are values")
// DATABROKER: databroker/src/permissions.rs — `can_read(&self, path) -> Result<(), PermissionError>`
//             and databroker/src/broker.rs `Entry::validate_value_min_max`.
// EXPECT:  tests fail: `todo!()` panics
//
// Rust has no exceptions. A function that can fail says so in its return
// type: `Result<T, E>` is either `Ok(value)` or `Err(error)`. The caller
// *must* deal with both — you can't accidentally ignore the error case.
//
// YOUR TASK
//   Implement `parse_speed` and `describe`. Use `match` (no `?` yet — that's
//   the next exercise). Do not edit the tests.

// I AM NOT DONE

/// Parses a speed in km/h from user input like " 120 ".
/// - Surrounding whitespace is ignored.
/// - Must be a whole number that fits in a `u16`; otherwise
///   `Err(format!("not a number: {input}"))` (use the *original* input).
/// - Speeds above 400 are rejected with `Err("unrealistic speed: {n}".to_string())`.
fn parse_speed(input: &str) -> Result<u16, String> {
    todo!()
}

/// Turns the result into a log line: "ok: 120 km/h" or "error: <message>".
fn describe(result: &Result<u16, String>) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_whitespace() {
        assert_eq!(parse_speed(" 120 "), Ok(120));
        assert_eq!(parse_speed("0"), Ok(0));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_speed("fast"), Err("not a number: fast".to_string()));
        assert_eq!(parse_speed("-5"), Err("not a number: -5".to_string()));
        assert_eq!(parse_speed("70000"), Err("not a number: 70000".to_string()));
    }

    #[test]
    fn rejects_unrealistic() {
        assert_eq!(
            parse_speed("401"),
            Err("unrealistic speed: 401".to_string())
        );
        assert_eq!(parse_speed("400"), Ok(400));
    }

    #[test]
    fn describes_both_cases() {
        assert_eq!(describe(&parse_speed("90")), "ok: 90 km/h");
        assert_eq!(describe(&parse_speed("x")), "error: not a number: x");
    }
}
