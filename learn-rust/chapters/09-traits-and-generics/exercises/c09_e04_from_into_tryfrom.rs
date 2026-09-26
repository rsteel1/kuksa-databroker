// c09_e04_from_into_tryfrom.rs
//
// CONCEPT: conversion traits: `From`/`Into` (infallible) and
//          `TryFrom`/`TryInto` (fallible), and why you implement `From`.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Conversions: From, Into, TryFrom")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs —
//             `impl From<proto::Value> for broker::DataValue` and
//             `impl From<broker::DataValue> for proto::Value`. Note its
//             `None => todo!()` arm: a client sending an empty Value would
//             panic that request's task. `TryFrom` is the honest signature.
// EXPECT:  tests fail: `todo!()` panics
//
// The gRPC layer receives prost-generated structs and must convert them to
// the broker's own types, and back. Here `WireValue` mimics the generated
// `proto::Value` (a struct wrapping an optional "oneof" enum).
//
// YOUR TASK
//   1. `impl From<DataValue> for WireValue` (every DataValue has a wire form).
//   2. `impl TryFrom<WireValue> for DataValue` — an empty WireValue is an
//      error (`ConversionError::Empty`), not a panic.
//   3. `impl TryFrom<&str> for DataType` for VSS type names:
//      "boolean", "int32", "string", "int32[]" -> Ok; anything else ->
//      `Err(ConversionError::UnknownType(name.to_string()))`.
//   Only implement `From` / `TryFrom`; `.into()` / `.try_into()` in the
//   tests come for free. Do not edit the tests.

// I AM NOT DONE

// (TryFrom/TryInto are in the 2021 prelude — no `use` needed.)

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Bool(bool),
    Int32(i32),
    String(String),
    Int32Array(Vec<i32>),
}

#[derive(Debug, Clone, PartialEq)]
enum DataType {
    Bool,
    Int32,
    String,
    Int32Array,
}

/// Mimics prost's generated `proto::Value { typed_value: Option<TypedValue> }`.
#[derive(Debug, Clone, PartialEq, Default)]
struct WireValue {
    typed_value: Option<TypedValue>,
}

#[derive(Debug, Clone, PartialEq)]
enum TypedValue {
    Bool(bool),
    Int32(i32),
    String(String),
    Int32Array(Vec<i32>),
}

#[derive(Debug, PartialEq)]
enum ConversionError {
    Empty,
    UnknownType(String),
}

impl From<DataValue> for WireValue {
    fn from(value: DataValue) -> Self {
        todo!()
    }
}

impl TryFrom<WireValue> for DataValue {
    type Error = ConversionError;

    fn try_from(wire: WireValue) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<&str> for DataType {
    type Error = ConversionError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_wire_via_from() {
        let wire: WireValue = DataValue::Int32(5).into();
        assert_eq!(wire.typed_value, Some(TypedValue::Int32(5)));
        let wire = WireValue::from(DataValue::String("x".into()));
        assert_eq!(wire.typed_value, Some(TypedValue::String("x".into())));
    }

    #[test]
    fn round_trip() {
        for v in [
            DataValue::Bool(true),
            DataValue::Int32(-1),
            DataValue::String("hi".into()),
            DataValue::Int32Array(vec![1, 2]),
        ] {
            let wire: WireValue = v.clone().into();
            let back: DataValue = wire.try_into().unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn empty_wire_value_is_an_error_not_a_panic() {
        assert_eq!(
            DataValue::try_from(WireValue::default()),
            Err(ConversionError::Empty)
        );
    }

    #[test]
    fn data_type_names() {
        assert_eq!(DataType::try_from("int32"), Ok(DataType::Int32));
        let t: Result<DataType, _> = "int32[]".try_into();
        assert_eq!(t, Ok(DataType::Int32Array));
        assert_eq!(DataType::try_from("boolean"), Ok(DataType::Bool));
        assert_eq!(DataType::try_from("string"), Ok(DataType::String));
        assert_eq!(
            DataType::try_from("int128"),
            Err(ConversionError::UnknownType("int128".into()))
        );
    }
}
