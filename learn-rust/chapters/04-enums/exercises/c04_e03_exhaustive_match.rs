// c04_e03_exhaustive_match.rs
//
// CONCEPT: exhaustiveness checking, and why the `_` wildcard can hide bugs.
// GUIDE:   chapters/04-enums/guide.md  (section "Exhaustive matching and the wildcard trap")
// DATABROKER: databroker/src/types.rs — `impl fmt::Display for DataType`
//             lists all 24 variants explicitly, one arm each.
// EXPECT:  compile error E0004 (non-exhaustive patterns: `DataType::Uint8`
//          not covered), then a failing test in `is_array`.
//
// `match` must handle every variant. That's a superpower: add a new variant
// to an enum and the compiler shows you every place that must be updated —
// UNLESS a `_ => ...` arm silently swallows it. `is_array` below was written
// before `BoolArray` existed, and the wildcard hid the new variant.
//
// YOUR TASK
//   1. Fix `Display` for `DataType` (E0004).
//   2. Fix `is_array` by listing every variant explicitly — no `_` arm — so
//      the compiler will catch the next variant someone adds.
//   3. Implement `element_type`.
//   Do not edit the tests.

// I AM NOT DONE

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataType {
    String,
    Bool,
    Int32,
    Uint8,
    Double,
    StringArray,
    BoolArray,
    Int32Array,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataType::String => write!(f, "String"),
            DataType::Bool => write!(f, "Bool"),
            DataType::Int32 => write!(f, "Int32"),
            DataType::Double => write!(f, "Double"),
            DataType::StringArray => write!(f, "StringArray"),
            DataType::BoolArray => write!(f, "BoolArray"),
            DataType::Int32Array => write!(f, "Int32Array"),
        }
    }
}

fn is_array(data_type: DataType) -> bool {
    match data_type {
        DataType::StringArray | DataType::Int32Array => true,
        _ => false,
    }
}

/// For an array type, the type of one element; for scalars, `None`.
fn element_type(data_type: DataType) -> Option<DataType> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(DataType::Uint8.to_string(), "Uint8");
        assert_eq!(format!("{}", DataType::BoolArray), "BoolArray");
    }

    #[test]
    fn arrays() {
        assert!(is_array(DataType::StringArray));
        assert!(is_array(DataType::Int32Array));
        assert!(is_array(DataType::BoolArray));
        assert!(!is_array(DataType::Bool));
        assert!(!is_array(DataType::Uint8));
    }

    #[test]
    fn elements() {
        assert_eq!(element_type(DataType::BoolArray), Some(DataType::Bool));
        assert_eq!(element_type(DataType::StringArray), Some(DataType::String));
        assert_eq!(element_type(DataType::Int32Array), Some(DataType::Int32));
        assert_eq!(element_type(DataType::Double), None);
    }
}
