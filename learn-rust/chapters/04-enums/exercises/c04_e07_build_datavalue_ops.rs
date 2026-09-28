// c04_e07_build_datavalue_ops.rs
//
// CONCEPT: writing methods on an enum from a spec: matching on variants,
//          widening mixed numeric types through `Option`, and returning
//          a new value of the same enum.
// GUIDE:   chapters/04-enums/guide.md  (section "Matching two values at once")
// DATABROKER: databroker/src/types.rs — `DataValue`'s numeric variants and
//             `greater_than`, which compares them after widening to `f64`.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. Implement each method so every test passes.
//
// YOUR TASK
//   Implement `type_name`, `as_f64`, `greater_than` and `clamp_numeric`
//   on `DataValue`.
//   Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl DataValue {
    /// `"bool"`, `"int"`, `"float"` or `"text"`.
    fn type_name(&self) -> &'static str {
        todo!()
    }

    /// Widens `Int`/`Float` to `f64`. `None` for `Bool`/`Text`.
    fn as_f64(&self) -> Option<f64> {
        todo!()
    }

    /// `Some(true/false)` if both sides widen to a number (an `Int` and
    /// a `Float` may be compared with each other). `None` if either side
    /// is a `Bool` or a `Text`.
    fn greater_than(&self, other: &DataValue) -> Option<bool> {
        todo!()
    }

    /// Clamps a numeric value into `min..=max`. `Bool` and `Text` are
    /// returned unchanged. An `Int` stays an `Int` (rounded towards zero
    /// after clamping); a `Float` stays a `Float`.
    fn clamp_numeric(self, min: f64, max: f64) -> DataValue {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_names() {
        assert_eq!(DataValue::Bool(true).type_name(), "bool");
        assert_eq!(DataValue::Int(5).type_name(), "int");
        assert_eq!(DataValue::Float(5.0).type_name(), "float");
        assert_eq!(DataValue::Text("x".to_string()).type_name(), "text");
    }

    #[test]
    fn widen_to_f64() {
        assert_eq!(DataValue::Int(5).as_f64(), Some(5.0));
        assert_eq!(DataValue::Float(2.5).as_f64(), Some(2.5));
        assert_eq!(DataValue::Bool(true).as_f64(), None);
        assert_eq!(DataValue::Text("x".to_string()).as_f64(), None);
    }

    #[test]
    fn greater_than_widens_mixed_numeric_types() {
        assert_eq!(
            DataValue::Int(10).greater_than(&DataValue::Float(5.5)),
            Some(true)
        );
        assert_eq!(
            DataValue::Float(1.0).greater_than(&DataValue::Int(2)),
            Some(false)
        );
        assert_eq!(
            DataValue::Text("a".to_string()).greater_than(&DataValue::Int(2)),
            None
        );
    }

    #[test]
    fn clamp_numeric_only_touches_numbers() {
        assert_eq!(
            DataValue::Int(500).clamp_numeric(0.0, 100.0),
            DataValue::Int(100)
        );
        assert_eq!(
            DataValue::Int(-5).clamp_numeric(0.0, 100.0),
            DataValue::Int(0)
        );
        assert_eq!(
            DataValue::Float(50.5).clamp_numeric(0.0, 100.0),
            DataValue::Float(50.5)
        );
        assert_eq!(
            DataValue::Bool(true).clamp_numeric(0.0, 100.0),
            DataValue::Bool(true)
        );
        assert_eq!(
            DataValue::Text("hi".to_string()).clamp_numeric(0.0, 100.0),
            DataValue::Text("hi".to_string())
        );
    }
}
