// c07_e04_from_conversions.rs
//
// CONCEPT: `?` converts errors with `From`; one error type for a whole module.
// GUIDE:   chapters/07-errors/guide.md  (section "How ? converts errors: From")
// DATABROKER: databroker/src/vss.rs — `impl From<serde_json::Error> for Error`
//             lets `parse_vss_from_reader` write `serde_json::from_reader(reader)?`.
// EXPECT:  compile error E0277 ("`?` couldn't convert the error to `VssError`")
//
// When you write `x?` in a function returning `Result<T, MyError>` and `x`
// is `Result<U, OtherError>`, Rust calls `MyError::from(other_error)`.
// If there's no `impl From<OtherError> for MyError`, it doesn't compile.
//
// YOUR TASK
//   Add the two missing `From` impls so every `?` in `max_from_json`
//   compiles, then make the tests pass. Do not edit `max_from_json` or the tests.

use std::num::ParseIntError;

#[derive(Debug, PartialEq)]
enum VssError {
    /// The document is not valid JSON.
    Json(String),
    /// A required field is missing.
    MissingField(&'static str),
    /// A field is present but not a valid integer.
    BadNumber(String),
}

impl From<serde_json::Error> for VssError {
    fn from(err: serde_json::Error) -> Self {
        VssError::Json(err.to_string())
    }
}

impl From<ParseIntError> for VssError {
    fn from(err: ParseIntError) -> Self {
        VssError::BadNumber(err.to_string())
    }
}

/// VSS files sometimes store numbers as strings: {"max": "250"}.
/// Reads the "max" field and parses it as an i64.
fn max_from_json(json: &str) -> Result<i64, VssError> {
    let doc: serde_json::Value = serde_json::from_str(json)?; // serde_json::Error -> VssError
    let text = doc
        .get("max")
        .and_then(|v| v.as_str())
        .ok_or(VssError::MissingField("max"))?; // already a VssError
    let max = text.trim().parse::<i64>()?; // ParseIntError -> VssError
    Ok(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_max() {
        assert_eq!(max_from_json(r#"{"max": "250"}"#), Ok(250));
        assert_eq!(max_from_json(r#"{"max": " -3 "}"#), Ok(-3));
    }

    #[test]
    fn invalid_json() {
        assert!(matches!(max_from_json("{not json"), Err(VssError::Json(_))));
    }

    #[test]
    fn missing_field() {
        assert_eq!(
            max_from_json(r#"{"min": "0"}"#),
            Err(VssError::MissingField("max"))
        );
        // A number (not a string) also counts as missing for this format.
        assert_eq!(
            max_from_json(r#"{"max": 5}"#),
            Err(VssError::MissingField("max"))
        );
    }

    #[test]
    fn bad_number_keeps_the_parse_message() {
        assert_eq!(
            max_from_json(r#"{"max": "lots"}"#),
            Err(VssError::BadNumber(
                "invalid digit found in string".to_string()
            ))
        );
    }
}
