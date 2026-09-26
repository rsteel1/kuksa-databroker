// c30_e03_proto_conversions.rs
//
// CONCEPT: `From` impls at the API boundary — wire types <-> domain types, and the orphan rule.
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "Converting at the boundary")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs —
//             `impl From<proto::Value> for broker::DataValue`,
//             `impl From<broker::DataValue> for proto::Value`,
//             `impl From<broker::Datapoint> for Option<proto::Datapoint>`
// EXPECT:  tests fail: `todo!()` panics
//
// The broker core never touches protobuf types. Every gRPC handler converts
// the request into broker types on the way in, and broker results into
// protobuf types on the way out, via `From`/`Into`. The API version (v1/v2)
// is then just a thin translation layer on top of one broker.
//
// The ORPHAN RULE: you may implement a trait for a type only if the trait or
// the type is defined in your crate. That's why you can't write
// `impl From<proto::Value> for databroker::types::DataValue` here — both are
// foreign (and the databroker already did it). We use our own `DataValue`.
// `impl From<DataValue> for Option<proto::Value>` IS allowed, because the
// trait's type parameter (`DataValue`) is local.
//
// Mapping rules:
//   * proto Value with `typed_value: None`  <->  DataValue::NotAvailable
//   * arrays are wrapped in their own message: TypedValue::Int32Array(proto::Int32Array { values })
//   * NotAvailable -> `None` when converting to `Option<proto::Value>`
//
// YOUR TASK
//   Implement the three `From` impls. Do not edit the tests.

use databroker_proto::kuksa::val::v2 as proto;
use proto::value::TypedValue;

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    NotAvailable,
    Bool(bool),
    String(String),
    Int32(i32),
    Uint64(u64),
    Double(f64),
    StringArray(Vec<String>),
    Int32Array(Vec<i32>),
}

impl From<proto::Value> for DataValue {
    fn from(value: proto::Value) -> Self {
        // WHY: we own `value`, so we can move the Strings/Vecs out instead of
        // cloning them (the databroker matches on a reference and clones).
        match value.typed_value {
            None => DataValue::NotAvailable,
            Some(TypedValue::Bool(v)) => DataValue::Bool(v),
            Some(TypedValue::String(v)) => DataValue::String(v),
            Some(TypedValue::Int32(v)) => DataValue::Int32(v),
            Some(TypedValue::Uint64(v)) => DataValue::Uint64(v),
            Some(TypedValue::Double(v)) => DataValue::Double(v),
            Some(TypedValue::StringArray(array)) => DataValue::StringArray(array.values),
            Some(TypedValue::Int32Array(array)) => DataValue::Int32Array(array.values),
            // Types this exercise's DataValue doesn't model:
            Some(_) => DataValue::NotAvailable,
        }
    }
}

impl From<DataValue> for proto::Value {
    fn from(value: DataValue) -> Self {
        let typed_value = match value {
            DataValue::NotAvailable => None,
            DataValue::Bool(v) => Some(TypedValue::Bool(v)),
            DataValue::String(v) => Some(TypedValue::String(v)),
            DataValue::Int32(v) => Some(TypedValue::Int32(v)),
            DataValue::Uint64(v) => Some(TypedValue::Uint64(v)),
            DataValue::Double(v) => Some(TypedValue::Double(v)),
            DataValue::StringArray(values) => {
                Some(TypedValue::StringArray(proto::StringArray { values }))
            }
            DataValue::Int32Array(values) => {
                Some(TypedValue::Int32Array(proto::Int32Array { values }))
            }
        };
        proto::Value { typed_value }
    }
}

impl From<DataValue> for Option<proto::Value> {
    fn from(value: DataValue) -> Self {
        match value {
            DataValue::NotAvailable => None,
            other => Some(other.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<DataValue> {
        vec![
            DataValue::Bool(true),
            DataValue::String("hello".into()),
            DataValue::Int32(-7),
            DataValue::Uint64(u64::MAX),
            DataValue::Double(0.25),
            DataValue::StringArray(vec!["LEFT".into(), "RIGHT".into()]),
            DataValue::Int32Array(vec![1, 2, 3]),
        ]
    }

    #[test]
    fn round_trip_every_variant() {
        for v in samples().into_iter().chain([DataValue::NotAvailable]) {
            let wire: proto::Value = v.clone().into();
            let back: DataValue = wire.into();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn arrays_use_wrapper_messages() {
        let wire: proto::Value = DataValue::Int32Array(vec![4, 5]).into();
        assert_eq!(
            wire.typed_value,
            Some(TypedValue::Int32Array(proto::Int32Array {
                values: vec![4, 5]
            }))
        );
    }

    #[test]
    fn not_available_is_an_empty_value_or_none() {
        let wire: proto::Value = DataValue::NotAvailable.into();
        assert_eq!(wire.typed_value, None);
        let optional: Option<proto::Value> = DataValue::NotAvailable.into();
        assert_eq!(optional, None);
        let optional: Option<proto::Value> = DataValue::Int32(1).into();
        assert!(optional.is_some());
    }

    #[test]
    fn agrees_with_the_real_databroker() {
        // The databroker's own conversion (conversions.rs) is in scope simply
        // because trait impls are global once the crate is linked.
        for v in samples() {
            let wire: proto::Value = v.clone().into();
            let theirs = databroker::types::DataValue::from(wire);
            assert_eq!(format!("{theirs:?}"), format!("{v:?}"));
        }
    }
}
