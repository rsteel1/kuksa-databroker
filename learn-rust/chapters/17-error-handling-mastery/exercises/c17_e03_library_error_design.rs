// c17_e03_library_error_design.rs
//
// CONCEPT: designing the public error type of a library: one enum per API
//          boundary, a variant per thing the *caller* might handle
//          differently, structured fields (not pre-formatted Strings), the
//          underlying cause kept as a `source`, a coarse `kind()` for callers
//          who only care about the category, `#[non_exhaustive]` so you can
//          add variants later, and `Send + Sync + 'static` so the error can
//          cross threads and be boxed.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Designing a library error type")
// DATABROKER: databroker/src/broker.rs:64 `UpdateError` (a good start: one
//             variant per failure) vs broker.rs:189-193 `Result<(), (ActuationError, String)>`
//             (a tuple of enum + free-text String: the String can't be
//             matched on, and nothing stops two call sites from phrasing the
//             same failure differently).
// EXPECT:  compile error E0599 / E0277: `StoreError` doesn't implement
//          `Display` or `std::error::Error` yet
//
// Error message conventions (Rust API Guidelines, C-GOOD-ERR):
//   * lowercase, no trailing punctuation: "signal not found: Vehicle.Speed"
//   * say what went wrong, not what the caller should do
//   * don't include the cause's text in your own message: it's already
//     available through `source()`, and reports print the whole chain
//
// YOUR TASK
//   1. Derive `thiserror::Error` for `StoreError` with the messages the
//      tests expect, keeping the ParseIntError as the `source`.
//   2. Implement `StoreError::kind`.
//   3. Implement `SignalStore::set` and `SignalStore::set_from_str`.
//   Do not edit the tests.

// I AM NOT DONE

mod store {
    use std::collections::HashMap;
    use std::num::ParseIntError;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ValueType {
        Int,
        Bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        Int(i64),
        Bool(bool),
    }

    impl Value {
        pub fn value_type(&self) -> ValueType {
            match self {
                Value::Int(_) => ValueType::Int,
                Value::Bool(_) => ValueType::Bool,
            }
        }
    }

    /// Coarse category, like `std::io::ErrorKind`: lets callers branch on
    /// "what sort of problem" without matching every variant.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorKind {
        NotFound,
        InvalidInput,
    }

    #[derive(Debug)]
    #[non_exhaustive]
    pub enum StoreError {
        NotFound {
            path: String,
        },
        WrongType {
            path: String,
            expected: ValueType,
            actual: ValueType,
        },
        OutOfRange {
            path: String,
            value: i64,
            min: i64,
            max: i64,
        },
        Parse {
            path: String,
            input: String,
            cause: ParseIntError,
        },
    }

    impl StoreError {
        /// NotFound -> ErrorKind::NotFound; everything else is the caller's
        /// input being wrong -> ErrorKind::InvalidInput.
        pub fn kind(&self) -> ErrorKind {
            todo!()
        }
    }

    struct Entry {
        value: Value,
        min: i64,
        max: i64,
    }

    #[derive(Default)]
    pub struct SignalStore {
        entries: HashMap<String, Entry>,
    }

    impl SignalStore {
        pub fn register(&mut self, path: &str, initial: Value, min: i64, max: i64) {
            self.entries.insert(
                path.to_string(),
                Entry {
                    value: initial,
                    min,
                    max,
                },
            );
        }

        pub fn get(&self, path: &str) -> Result<&Value, StoreError> {
            self.entries
                .get(path)
                .map(|e| &e.value)
                .ok_or_else(|| StoreError::NotFound {
                    path: path.to_string(),
                })
        }

        /// Checks, in order: the path exists (NotFound), the type matches
        /// the registered value's type (WrongType), and for Int values that
        /// min <= v <= max (OutOfRange). Only then stores the value.
        pub fn set(&mut self, path: &str, value: Value) -> Result<(), StoreError> {
            todo!()
        }

        /// Parses `input` as an i64 (after trimming) and calls `set` with it.
        /// A parse failure is `StoreError::Parse` with the ParseIntError
        /// kept as its source (not flattened into a String!).
        pub fn set_from_str(&mut self, path: &str, input: &str) -> Result<(), StoreError> {
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::store::*;
    use std::error::Error;

    fn assert_good_error_type<E: Error + Send + Sync + 'static>() {}

    fn store() -> SignalStore {
        let mut s = SignalStore::default();
        s.register("Vehicle.Speed", Value::Int(0), 0, 250);
        s.register("Vehicle.IsMoving", Value::Bool(false), 0, 0);
        s
    }

    #[test]
    fn is_a_proper_error_type() {
        assert_good_error_type::<StoreError>();
    }

    #[test]
    fn not_found() {
        let err = store().set("Vehicle.Nope", Value::Int(1)).unwrap_err();
        assert!(matches!(&err, StoreError::NotFound { path } if path == "Vehicle.Nope"));
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "signal not found: Vehicle.Nope");
    }

    #[test]
    fn wrong_type() {
        let err = store().set("Vehicle.IsMoving", Value::Int(1)).unwrap_err();
        assert!(matches!(
            err,
            StoreError::WrongType {
                expected: ValueType::Bool,
                actual: ValueType::Int,
                ..
            }
        ));
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "wrong type for Vehicle.IsMoving: expected Bool, got Int"
        );
    }

    #[test]
    fn out_of_range() {
        let mut s = store();
        let err = s.set("Vehicle.Speed", Value::Int(251)).unwrap_err();
        assert!(matches!(
            err,
            StoreError::OutOfRange {
                value: 251,
                min: 0,
                max: 250,
                ..
            }
        ));
        assert_eq!(
            err.to_string(),
            "value 251 for Vehicle.Speed is outside 0..=250"
        );
        // Nothing was stored.
        assert_eq!(s.get("Vehicle.Speed").unwrap(), &Value::Int(0));
    }

    #[test]
    fn parse_keeps_the_cause() {
        let err = store().set_from_str("Vehicle.Speed", "fast").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            "cannot parse \"fast\" as a value for Vehicle.Speed"
        );
        let cause = err.source().expect("the ParseIntError is the source");
        assert!(cause.downcast_ref::<std::num::ParseIntError>().is_some());
    }

    #[test]
    fn happy_paths() {
        let mut s = store();
        s.set("Vehicle.Speed", Value::Int(120)).unwrap();
        s.set_from_str("Vehicle.Speed", " 90 ").unwrap();
        s.set("Vehicle.IsMoving", Value::Bool(true)).unwrap();
        assert_eq!(s.get("Vehicle.Speed").unwrap(), &Value::Int(90));
        assert_eq!(s.get("Vehicle.IsMoving").unwrap(), &Value::Bool(true));
    }
}
