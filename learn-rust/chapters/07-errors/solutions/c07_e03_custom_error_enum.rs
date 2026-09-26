// c07_e03_custom_error_enum.rs
//
// CONCEPT: custom error enums + `Display` + `std::error::Error`.
// GUIDE:   chapters/07-errors/guide.md  (section "Designing an error type")
// DATABROKER: databroker/src/broker.rs — `pub enum UpdateError { NotFound,
//             WrongType, OutOfBoundsAllowed, OutOfBoundsMinMax, ... }` and
//             databroker/src/vss.rs `pub enum Error` with its `Display` impl.
// EXPECT:  compile errors E0599 (`UpdateError` doesn't implement `std::fmt::Display`)
//          and E0277 (`UpdateError: std::error::Error` not satisfied), then failing tests.
//
// A `String` error is fine for a quick script, but callers can't *match* on
// it. An enum lets the gRPC layer turn `NotFound` into a NOT_FOUND status and
// `OutOfBounds` into INVALID_ARGUMENT. To be a "proper" error it should also
// implement `Display` (human message) and `std::error::Error` (so it can be
// boxed as `Box<dyn Error>` and chained).
//
// YOUR TASK
//   1. Implement `fmt::Display` for `UpdateError` with the exact messages
//      listed on each variant.
//   2. Implement `std::error::Error` for it (an empty impl is enough).
//   3. Implement `validate`.
//   Do not edit the tests.

use std::fmt;

#[derive(Debug, PartialEq)]
enum UpdateError {
    /// "signal not found: <path>"
    NotFound { path: String },
    /// "wrong type: expected <expected>, got <actual>"
    WrongType {
        expected: &'static str,
        actual: &'static str,
    },
    /// "value <value> outside [<min>, <max>]"
    OutOfBounds { value: i64, min: i64, max: i64 },
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::NotFound { path } => write!(f, "signal not found: {path}"),
            UpdateError::WrongType { expected, actual } => {
                write!(f, "wrong type: expected {expected}, got {actual}")
            }
            UpdateError::OutOfBounds { value, min, max } => {
                write!(f, "value {value} outside [{min}, {max}]")
            }
        }
    }
}

// WHY: the default methods of `Error` (source, description) are fine here.
impl std::error::Error for UpdateError {}

/// Checks a proposed integer value against the signal's bounds (inclusive).
/// Unknown paths (anything not starting with "Vehicle.") -> NotFound.
fn validate(path: &str, value: i64, min: i64, max: i64) -> Result<(), UpdateError> {
    if !path.starts_with("Vehicle.") {
        return Err(UpdateError::NotFound {
            path: path.to_string(),
        });
    }
    if !(min..=max).contains(&value) {
        return Err(UpdateError::OutOfBounds { value, min, max });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        let e = UpdateError::NotFound {
            path: "Vehicle.Nope".into(),
        };
        assert_eq!(e.to_string(), "signal not found: Vehicle.Nope");
        let e = UpdateError::WrongType {
            expected: "int8",
            actual: "string",
        };
        assert_eq!(e.to_string(), "wrong type: expected int8, got string");
        let e = UpdateError::OutOfBounds {
            value: 300,
            min: -128,
            max: 127,
        };
        assert_eq!(e.to_string(), "value 300 outside [-128, 127]");
    }

    #[test]
    fn works_as_box_dyn_error() {
        // Only compiles if UpdateError implements std::error::Error.
        let boxed: Box<dyn std::error::Error> =
            Box::new(UpdateError::NotFound { path: "X".into() });
        assert_eq!(boxed.to_string(), "signal not found: X");
    }

    #[test]
    fn validate_rules() {
        assert_eq!(validate("Vehicle.Speed", 50, 0, 250), Ok(()));
        assert_eq!(validate("Vehicle.Speed", 250, 0, 250), Ok(()));
        assert_eq!(
            validate("Vehicle.Speed", 251, 0, 250),
            Err(UpdateError::OutOfBounds {
                value: 251,
                min: 0,
                max: 250
            })
        );
        assert_eq!(
            validate("Car.Speed", 1, 0, 10),
            Err(UpdateError::NotFound {
                path: "Car.Speed".into()
            })
        );
    }
}
