// c30_e02_oneof_enums.rs
//
// CONCEPT: protobuf `oneof` -> Rust enum; protobuf `enum` fields -> `i32` + accessors.
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "oneof and enums")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `get_signal` (matches on
//             `proto::signal_id::Signal::{Path, Id}`); conversions.rs `impl From<broker::DataType> for proto::DataType`
// EXPECT:  tests fail: `todo!()` panics
//
// In val.proto:
//     message SignalID { oneof signal { int32 id = 1; string path = 2; } }
// prost generates a struct with ONE field, `signal: Option<signal_id::Signal>`,
// and a nested module `signal_id` holding `enum Signal { Id(i32), Path(String) }`.
// The outer `Option` is `None` when the client set neither.
//
// Protobuf enums are different: on the wire they're just integers, and a
// newer client might send a value this server doesn't know. So prost stores
// the field as a raw `i32`, and generates helpers:
//     meta.data_type()               -> proto::DataType (unknown -> the default, Unspecified)
//     meta.set_data_type(DataType::Float)
//     proto::DataType::try_from(7)   -> Result<DataType, UnknownEnumValue>
//     DataType::Float.as_str_name()  -> "DATA_TYPE_FLOAT"
//
// YOUR TASK
//   Implement the functions below. Do not edit the tests.

// I AM NOT DONE

use databroker_proto::kuksa::val::v2 as proto;

fn by_id(id: i32) -> proto::SignalId {
    todo!()
}

fn by_path(path: impl Into<String>) -> proto::SignalId {
    todo!()
}

/// "id:5", "path:Vehicle.Speed" or "missing".
fn describe(signal_id: &proto::SignalId) -> String {
    todo!("match on &signal_id.signal")
}

/// Build Metadata for a float sensor using the typed setters (not raw ints).
fn float_sensor_metadata(path: &str, id: i32) -> proto::Metadata {
    todo!()
}

/// The protobuf name of the metadata's data type, e.g. "DATA_TYPE_FLOAT".
/// Unknown numbers must come out as "DATA_TYPE_UNSPECIFIED".
fn data_type_name(meta: &proto::Metadata) -> &'static str {
    todo!()
}

/// Strictly parse a raw enum number; `None` if this server doesn't know it.
fn parse_data_type(raw: i32) -> Option<proto::DataType> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_and_describing_ids() {
        assert_eq!(describe(&by_id(5)), "id:5");
        assert_eq!(describe(&by_path("Vehicle.Speed")), "path:Vehicle.Speed");
        assert_eq!(describe(&by_path(String::from("A.B"))), "path:A.B");
        assert_eq!(describe(&proto::SignalId::default()), "missing");
    }

    #[test]
    fn enums_are_stored_as_i32() {
        let meta = float_sensor_metadata("Vehicle.Speed", 12);
        assert_eq!(meta.data_type, proto::DataType::Float as i32);
        assert_eq!(meta.entry_type, proto::EntryType::Sensor as i32);
        assert_eq!(meta.data_type(), proto::DataType::Float);
        assert_eq!(meta.path, "Vehicle.Speed");
        assert_eq!(meta.id, 12);
    }

    #[test]
    fn enum_names() {
        let meta = float_sensor_metadata("Vehicle.Speed", 1);
        assert_eq!(data_type_name(&meta), "DATA_TYPE_FLOAT");

        let from_the_future = proto::Metadata {
            data_type: 999,
            ..Default::default()
        };
        assert_eq!(data_type_name(&from_the_future), "DATA_TYPE_UNSPECIFIED");
    }

    #[test]
    fn strict_parsing() {
        assert_eq!(
            parse_data_type(proto::DataType::Boolean as i32),
            Some(proto::DataType::Boolean)
        );
        assert_eq!(parse_data_type(999), None);
    }
}
