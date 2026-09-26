// c07_e02_question_mark.rs
//
// CONCEPT: the `?` operator; `ok_or`/`ok_or_else` to turn Option into Result.
// GUIDE:   chapters/07-errors/guide.md  (section "The ? operator")
// DATABROKER: databroker/src/main.rs — `read_metadata_file` uses `?` on file
//             open and VSS parsing; databroker/src/broker.rs `DatabaseWriteAccess::add`
//             uses `.map_err(...)?` on `can_create`.
// EXPECT:  compile error E0277 ("the `?` operator can only be used in a function
//          that returns `Result` or `Option`"), plus E0308 mismatched types
//          where the tests expect a Result.
//
// `expr?` means: "if this is Err(e), return Err(e) from my function right now;
// otherwise unwrap the Ok value". It only works in a function whose return
// type can carry that error.
//
// YOUR TASK
//   1. Fix the signature of `parse_assignment` so `?` is allowed. The tests
//      tell you the exact type.
//   2. Replace the `todo!()`s in `parse_assignment` with `ok_or`/`ok_or_else`
//      so a missing '=' and an empty path become errors.
//   Do not edit the tests.

/// Parses "Vehicle.Speed = 42" into ("Vehicle.Speed", 42).
/// Errors (exact strings, checked by the tests):
///   - no '='              -> "missing '='"
///   - empty path          -> "empty path"
///   - value not an i32    -> "bad value: <value as written, trimmed>"
fn parse_assignment(input: &str) -> Result<(String, i32), String> {
    let (path, value) = input.split_once('=').ok_or("missing '='")?;
    let path = path.trim();
    // WHY: `then_some` gives Some(path) when the condition holds, else None.
    let path = (!path.is_empty()).then_some(path).ok_or("empty path")?;
    let value = value.trim();
    let number = value
        .parse::<i32>()
        .map_err(|_| format!("bad value: {value}"))?;
    Ok((path.to_string(), number))
}

/// Parses several lines, stopping at the first error.
fn parse_all(lines: &[&str]) -> Result<Vec<(String, i32)>, String> {
    let mut out = Vec::new();
    for line in lines {
        out.push(parse_assignment(line)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ok() {
        assert_eq!(
            parse_assignment("Vehicle.Speed = 42"),
            Ok(("Vehicle.Speed".to_string(), 42))
        );
        assert_eq!(parse_assignment("A.B=-1"), Ok(("A.B".to_string(), -1)));
    }

    #[test]
    fn errors() {
        assert_eq!(
            parse_assignment("Vehicle.Speed 42"),
            Err("missing '='".to_string())
        );
        assert_eq!(parse_assignment("  = 42"), Err("empty path".to_string()));
        assert_eq!(
            parse_assignment("Vehicle.Speed = fast "),
            Err("bad value: fast".to_string())
        );
    }

    #[test]
    fn first_error_stops_parse_all() {
        assert_eq!(
            parse_all(&["A=1", "B=2"]),
            Ok(vec![("A".to_string(), 1), ("B".to_string(), 2)])
        );
        assert_eq!(
            parse_all(&["A=1", "B", "C=x"]),
            Err("missing '='".to_string())
        );
    }
}
