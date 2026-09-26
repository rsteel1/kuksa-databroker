// c04_e02_enum_with_data.rs
//
// CONCEPT: enum variants that carry data, and binding that data in patterns.
// GUIDE:   chapters/04-enums/guide.md  (section "Variants that carry data")
// DATABROKER: databroker/src/types.rs — `pub enum DataValue { NotAvailable,
//             Bool(bool), String(String), Int32(i32), ..., DoubleArray(Vec<f64>) }`
// EXPECT:  tests fail: `todo!()` panics.
//
// `DataValue` is the heart of the databroker: every signal's current value is
// one. Each variant holds a different type — and `match` is how you get the
// value back out, safely: you can only read the `f64` inside `Double(..)`
// after proving it IS a `Double`.
//
// YOUR TASK
//   Implement the three methods. Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    NotAvailable,
    Bool(bool),
    String(String),
    Int32(i32),
    Uint32(u32),
    Double(f64),
    Int32Array(Vec<i32>),
}

impl DataValue {
    /// A short type name: "NotAvailable", "Bool", "String", "Int32",
    /// "Uint32", "Double", "Int32Array".
    fn type_name(&self) -> &'static str {
        todo!()
    }

    /// Numeric scalar values as f64; everything else is `None`.
    fn as_f64(&self) -> Option<f64> {
        todo!()
    }

    /// How many values are held: arrays -> their length, NotAvailable -> 0,
    /// any scalar (including String) -> 1.
    fn len(&self) -> usize {
        todo!()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(DataValue::NotAvailable.type_name(), "NotAvailable");
        assert_eq!(DataValue::Bool(true).type_name(), "Bool");
        assert_eq!(DataValue::String("x".into()).type_name(), "String");
        assert_eq!(DataValue::Int32(1).type_name(), "Int32");
        assert_eq!(DataValue::Uint32(1).type_name(), "Uint32");
        assert_eq!(DataValue::Double(1.0).type_name(), "Double");
        assert_eq!(DataValue::Int32Array(vec![]).type_name(), "Int32Array");
    }

    #[test]
    fn numbers() {
        assert_eq!(DataValue::Int32(-3).as_f64(), Some(-3.0));
        assert_eq!(DataValue::Uint32(7).as_f64(), Some(7.0));
        assert_eq!(DataValue::Double(2.5).as_f64(), Some(2.5));
        assert_eq!(DataValue::Bool(true).as_f64(), None);
        assert_eq!(DataValue::String("12".into()).as_f64(), None);
        assert_eq!(DataValue::Int32Array(vec![1]).as_f64(), None);
    }

    #[test]
    fn lengths() {
        assert_eq!(DataValue::NotAvailable.len(), 0);
        assert!(DataValue::NotAvailable.is_empty());
        assert_eq!(DataValue::Double(1.0).len(), 1);
        assert_eq!(DataValue::String("hello".into()).len(), 1);
        assert_eq!(DataValue::Int32Array(vec![1, 2, 3]).len(), 3);
    }
}
