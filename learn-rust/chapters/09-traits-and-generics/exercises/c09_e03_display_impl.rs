// c09_e03_display_impl.rs
//
// CONCEPT: implementing std traits: `fmt::Display` (and seeing `Debug` vs `Display`).
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Implementing standard traits")
// DATABROKER: databroker/src/types.rs — `impl fmt::Display for DataType` and
//             `impl fmt::Display for DataValue` (arrays are printed with `{:?}`).
// EXPECT:  compile error E0277 (`DataValue` doesn't implement `std::fmt::Display`)
//
// `Debug` ({:?}) is for developers and can be derived. `Display` ({}) is for
// users and must be written by hand: only you know how a value should look
// to a human. Implementing `Display` also gives you `.to_string()` for free
// (via the blanket `impl<T: Display> ToString for T`).
//
// YOUR TASK
//   Implement `fmt::Display` for `DataType` and `DataValue`:
//     DataType:  "Bool", "Int32", "String", "Int32Array"  (like the real one)
//     DataValue: NotAvailable -> "Not Available"; scalars print their value;
//                strings print WITHOUT quotes; arrays print like `{:?}`
//                (e.g. "[1, 2, 3]" and "[\"a\", \"b\"]").
//   Do not edit the tests.

// I AM NOT DONE

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DataType {
    Bool,
    Int32,
    String,
    Int32Array,
}

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    NotAvailable,
    Bool(bool),
    Int32(i32),
    String(String),
    Int32Array(Vec<i32>),
    StringArray(Vec<String>),
}

// TODO: impl fmt::Display for DataType
// TODO: impl fmt::Display for DataValue

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types() {
        assert_eq!(DataType::Bool.to_string(), "Bool");
        assert_eq!(format!("{}", DataType::Int32Array), "Int32Array");
        assert_eq!(format!("type={}", DataType::String), "type=String");
    }

    #[test]
    fn values() {
        assert_eq!(DataValue::NotAvailable.to_string(), "Not Available");
        assert_eq!(DataValue::Bool(true).to_string(), "true");
        assert_eq!(DataValue::Int32(-7).to_string(), "-7");
        assert_eq!(DataValue::String("hello".into()).to_string(), "hello");
        assert_eq!(
            DataValue::Int32Array(vec![1, 2, 3]).to_string(),
            "[1, 2, 3]"
        );
        assert_eq!(
            DataValue::StringArray(vec!["a".into(), "b".into()]).to_string(),
            "[\"a\", \"b\"]"
        );
    }

    #[test]
    fn debug_is_different() {
        // Debug is derived and shows the structure, including quotes.
        assert_eq!(
            format!("{:?}", DataValue::String("hello".into())),
            "String(\"hello\")"
        );
    }
}
