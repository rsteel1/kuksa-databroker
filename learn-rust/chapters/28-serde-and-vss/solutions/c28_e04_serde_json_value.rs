// c28_e04_serde_json_value.rs
//
// CONCEPT: `serde_json::Value` for data whose type is only known at runtime.
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "serde_json::Value: typing data late")
// DATABROKER: databroker/src/vss.rs — `try_from_json_value`, `try_from_json_array`,
//             `impl From<serde_json::Error> for Error`
// EXPECT:  tests fail: `todo!()` panics
//
// A VSS entry's `default`, `min`, `max` and `allowed` fields can hold any JSON
// type: `"allowed": ["LEFT", "RIGHT"]`, `"max": 250`, `"default": false`.
// Their real type depends on the entry's `datatype`, which serde only sees as
// a sibling field. So vss.rs first reads them as untyped `serde_json::Value`
// and converts them *after* it knows the datatype, using
// `serde_json::from_value::<T>(value)`.
//
// Note how the databroker stores small integer types: int8/int16 become
// `DataValue::Int32`, uint8/uint16 become `DataValue::Uint32` — but the
// *range* of the declared type is still enforced while parsing (300 is not a
// valid int8!). `serde_json::from_value::<i8>` does that check for you.
//
// YOUR TASK
//   Implement `value_for_type` and `allowed_for_type`. Use `?` together with
//   the provided `From<serde_json::Error>` impl.
//   Do not edit the tests.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
enum DataType {
    Bool,
    String,
    Int8,
    Int32,
    Uint8,
    Float,
    StringArray,
    Int32Array,
}

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Bool(bool),
    String(String),
    Int32(i32),
    Uint32(u32),
    Float(f32),
    StringArray(Vec<String>),
    Int32Array(Vec<i32>),
}

#[derive(Debug)]
struct ParseError(String);

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        ParseError(err.to_string())
    }
}

/// Converts a JSON value into the `DataValue` matching `data_type` exactly.
/// A scalar type needs a scalar JSON value; an array type needs a JSON array.
fn value_for_type(value: Value, data_type: &DataType) -> Result<DataValue, ParseError> {
    let data_value = match data_type {
        DataType::Bool => DataValue::Bool(serde_json::from_value(value)?),
        DataType::String => DataValue::String(serde_json::from_value(value)?),
        // WHY: parse with the *declared* width so out-of-range values fail,
        // then widen losslessly with `.into()`.
        DataType::Int8 => DataValue::Int32(serde_json::from_value::<i8>(value)?.into()),
        DataType::Int32 => DataValue::Int32(serde_json::from_value(value)?),
        DataType::Uint8 => DataValue::Uint32(serde_json::from_value::<u8>(value)?.into()),
        DataType::Float => DataValue::Float(serde_json::from_value(value)?),
        DataType::StringArray => DataValue::StringArray(serde_json::from_value(value)?),
        DataType::Int32Array => DataValue::Int32Array(serde_json::from_value(value)?),
    };
    Ok(data_value)
}

/// `allowed` is always a JSON array in VSS, even for scalar signals. For a
/// scalar type the result is the *array* version of that type
/// (String -> StringArray, Int32 -> Int32Array). For types without an array
/// version here, return `ParseError`.
fn allowed_for_type(values: Vec<Value>, data_type: &DataType) -> Result<DataValue, ParseError> {
    let array_type = match data_type {
        DataType::String | DataType::StringArray => DataType::StringArray,
        DataType::Int32 | DataType::Int32Array => DataType::Int32Array,
        other => return Err(ParseError(format!("`allowed` not supported for {other:?}"))),
    };
    value_for_type(Value::Array(values), &array_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(
            value_for_type(json!(true), &DataType::Bool).unwrap(),
            DataValue::Bool(true)
        );
        assert_eq!(
            value_for_type(json!("abc"), &DataType::String).unwrap(),
            DataValue::String("abc".into())
        );
        assert_eq!(
            value_for_type(json!(250.5), &DataType::Float).unwrap(),
            DataValue::Float(250.5)
        );
    }

    #[test]
    fn small_ints_are_widened_but_range_checked() {
        assert_eq!(
            value_for_type(json!(-128), &DataType::Int8).unwrap(),
            DataValue::Int32(-128)
        );
        assert_eq!(
            value_for_type(json!(255), &DataType::Uint8).unwrap(),
            DataValue::Uint32(255)
        );
        assert!(value_for_type(json!(300), &DataType::Int8).is_err());
        assert!(value_for_type(json!(-1), &DataType::Uint8).is_err());
    }

    #[test]
    fn wrong_json_type_is_an_error_with_a_message() {
        let err = value_for_type(json!("fast"), &DataType::Int32).unwrap_err();
        assert!(err.0.contains("invalid type"), "got: {}", err.0);
        // scalar where an array is expected, and vice versa
        assert!(value_for_type(json!(1), &DataType::Int32Array).is_err());
        assert!(value_for_type(json!([1, 2]), &DataType::Int32).is_err());
    }

    #[test]
    fn arrays() {
        assert_eq!(
            value_for_type(json!(["a", "b"]), &DataType::StringArray).unwrap(),
            DataValue::StringArray(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn allowed_values_become_arrays() {
        let allowed = vec![json!("LEFT"), json!("RIGHT")];
        assert_eq!(
            allowed_for_type(allowed, &DataType::String).unwrap(),
            DataValue::StringArray(vec!["LEFT".into(), "RIGHT".into()])
        );
        assert_eq!(
            allowed_for_type(vec![json!(1), json!(2)], &DataType::Int32).unwrap(),
            DataValue::Int32Array(vec![1, 2])
        );
        assert!(allowed_for_type(vec![json!(true)], &DataType::Bool).is_err());
        assert!(allowed_for_type(vec![json!("x")], &DataType::Int32).is_err());
    }
}
