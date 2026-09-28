// c30_e07_review_proto_field_mapping.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs typical
//          of hand-written protobuf conversions: a signed/unsigned pair
//          swapped in an enum mapping, a value routed into the wrong
//          `oneof` field of the same "kind" (numeric), and a narrowing cast
//          that silently loses precision.
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "Value ↔ DataValue: both directions")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs:314-343 —
//             `impl From<broker::DataType> for proto::DataType` (one arm per
//             variant, easy to swap two by copy-paste); conversions.rs:436-475
//             — `impl From<broker::DataValue> for proto::Value` builds the
//             `oneof typed_value`, one variant per numeric width; the
//             opposite direction is at conversions.rs:394-431.
// EXPECT:  tests fail: `DataType::Int8` reports itself as `Uint8` (and vice
//          versa), a large `u32` becomes a negative number after the round
//          trip, and a `Double` loses precision after the round trip.
//
// The types below mirror a slice of `kuksa.val.v2.DataType` and the
// `typed_value` oneof in `kuksa.val.v2.Value`: `Data*` is the databroker's
// internal type, `Proto*` is the wire type prost generates.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not rename any variant, and do not edit the tests.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataType {
    Int8,
    Uint8,
    Int32,
    Uint32,
    Float,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtoDataType {
    Int8,
    Uint8,
    Int32,
    Uint32,
    Float,
    Double,
}

/// Mirrors `impl From<broker::DataType> for proto::DataType`.
fn to_proto_data_type(dt: DataType) -> ProtoDataType {
    match dt {
        // BUG 1: `Int8` and `Uint8` were swapped, almost certainly a
        // copy-paste slip between two adjacent match arms. Every signed
        // 8-bit signal would report itself as unsigned on the wire, and
        // vice versa.
        DataType::Int8 => ProtoDataType::Int8,
        DataType::Uint8 => ProtoDataType::Uint8,
        DataType::Int32 => ProtoDataType::Int32,
        DataType::Uint32 => ProtoDataType::Uint32,
        DataType::Float => ProtoDataType::Float,
        DataType::Double => ProtoDataType::Double,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Int32(i32),
    Uint32(u32),
    Float(f32),
    Double(f64),
}

/// Mirrors the `oneof typed_value` fields of `kuksa.val.v2.Value`.
#[derive(Debug, Clone, PartialEq)]
enum ProtoValue {
    Int32(i32),
    Uint32(u32),
    Float(f32),
    Double(f64),
}

/// Mirrors `impl From<broker::DataValue> for proto::Value`.
fn to_proto_value(value: DataValue) -> ProtoValue {
    match value {
        DataValue::Int32(v) => ProtoValue::Int32(v),
        // BUG 2: a `Uint32` was routed into the `int32` oneof field instead
        // of `uint32`. The cast `as i32` compiles (both are 32-bit), so
        // nothing warns you, but any value above `i32::MAX` comes out
        // negative on the wire.
        DataValue::Uint32(v) => ProtoValue::Uint32(v),
        DataValue::Float(v) => ProtoValue::Float(v),
        // BUG 3: a `Double` was routed into the `float` oneof field instead
        // of `double`. `as f32` compiles too, but silently rounds to
        // single precision.
        DataValue::Double(v) => ProtoValue::Double(v),
    }
}

/// Mirrors `impl From<proto::Value> for broker::DataValue`.
fn from_proto_value(value: ProtoValue) -> DataValue {
    match value {
        ProtoValue::Int32(v) => DataValue::Int32(v),
        ProtoValue::Uint32(v) => DataValue::Uint32(v),
        ProtoValue::Float(v) => DataValue::Float(v),
        ProtoValue::Double(v) => DataValue::Double(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int8_and_uint8_are_not_swapped() {
        assert_eq!(to_proto_data_type(DataType::Int8), ProtoDataType::Int8);
        assert_eq!(to_proto_data_type(DataType::Uint8), ProtoDataType::Uint8);
    }

    #[test]
    fn large_uint32_round_trips_through_the_uint32_field() {
        let original = DataValue::Uint32(4_000_000_000);
        let wire = to_proto_value(original.clone());
        assert_eq!(
            wire,
            ProtoValue::Uint32(4_000_000_000),
            "a u32 belongs in the wire type's uint32 field, not its int32 field"
        );
        assert_eq!(from_proto_value(wire), original);
    }

    #[test]
    fn double_round_trips_without_losing_precision() {
        let original = DataValue::Double(0.1 + 0.2);
        let wire = to_proto_value(original.clone());
        assert_eq!(
            wire,
            ProtoValue::Double(0.1 + 0.2),
            "a Double belongs in the wire type's double field, not its float field"
        );
        assert_eq!(from_proto_value(wire), original);
    }
}
