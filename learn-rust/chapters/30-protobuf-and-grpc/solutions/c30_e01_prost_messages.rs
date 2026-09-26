// c30_e01_prost_messages.rs
//
// CONCEPT: prost-generated message structs — building, defaults, encode/decode.
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "From .proto to Rust structs")
// DATABROKER: proto/kuksa/val/v2/types.proto + val.proto (the source);
//             databroker-proto/build.rs (generates Rust at build time);
//             databroker-proto/src/lib.rs (`tonic::include_proto!("kuksa.val.v2")`)
// EXPECT:  tests fail: `todo!()` panics
//
// Every message in val.proto becomes a plain Rust struct with public fields:
//
//   message Datapoint { google.protobuf.Timestamp timestamp = 1; Value value = 2; }
//     ->  pub struct Datapoint { pub timestamp: Option<prost_types::Timestamp>,
//                                pub value: Option<Value> }
//
// Message-typed fields are always `Option<_>` (protobuf can't tell "absent"
// from "default" for them), `repeated` becomes `Vec<_>`, `map<k,v>` becomes
// `HashMap<k,v>`, and every message implements `Default` + `prost::Message`
// (`encode_to_vec`, `decode`).
//
// YOUR TASK
//   Implement the four functions using the real generated types in
//   `databroker_proto::kuksa::val::v2` (imported as `proto`).
//   Tip: `prost_types::Timestamp` implements `From<SystemTime>`.
//   Do not edit the tests.

use databroker_proto::kuksa::val::v2 as proto;
use prost::Message;
use std::time::SystemTime;

/// A datapoint holding a float value and the given timestamp.
fn float_datapoint(value: f32, at: SystemTime) -> proto::Datapoint {
    proto::Datapoint {
        timestamp: Some(at.into()),
        value: Some(proto::Value {
            typed_value: Some(proto::value::TypedValue::Float(value)),
        }),
    }
}

/// A PublishValue request addressing the signal by *path*.
fn publish_by_path(path: &str, data_point: proto::Datapoint) -> proto::PublishValueRequest {
    proto::PublishValueRequest {
        signal_id: Some(proto::SignalId {
            signal: Some(proto::signal_id::Signal::Path(path.to_string())),
        }),
        data_point: Some(data_point),
    }
}

/// A Subscribe request for the given paths, with the given buffer size and
/// no filter. (`..Default::default()` is handy for fields you don't set.)
fn subscribe_request(paths: &[&str], buffer_size: u32) -> proto::SubscribeRequest {
    proto::SubscribeRequest {
        signal_paths: paths.iter().map(|p| p.to_string()).collect(),
        buffer_size,
        ..Default::default()
    }
}

/// Serialise to protobuf bytes and parse them back — what tonic does on the wire.
fn round_trip(
    request: &proto::PublishValueRequest,
) -> Result<proto::PublishValueRequest, prost::DecodeError> {
    let bytes = request.encode_to_vec();
    proto::PublishValueRequest::decode(bytes.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn t0() -> SystemTime {
        UNIX_EPOCH + Duration::new(1_700_000_000, 500)
    }

    #[test]
    fn datapoint_fields() {
        let dp = float_datapoint(88.5, t0());
        let ts = dp.timestamp.expect("timestamp set");
        assert_eq!(ts.seconds, 1_700_000_000);
        assert_eq!(ts.nanos, 500);
        assert_eq!(
            dp.value,
            Some(proto::Value {
                typed_value: Some(proto::value::TypedValue::Float(88.5))
            })
        );
    }

    #[test]
    fn default_message_has_nothing_set() {
        let empty = proto::Datapoint::default();
        assert_eq!(empty.timestamp, None);
        assert_eq!(empty.value, None);
        // An all-default message encodes to zero bytes!
        assert!(empty.encode_to_vec().is_empty());
    }

    #[test]
    fn publish_request_uses_path() {
        let req = publish_by_path("Vehicle.Speed", float_datapoint(1.0, t0()));
        assert_eq!(
            req.signal_id.and_then(|id| id.signal),
            Some(proto::signal_id::Signal::Path("Vehicle.Speed".into()))
        );
        assert!(req.data_point.is_some());
    }

    #[test]
    fn subscribe_request_defaults() {
        let req = subscribe_request(&["Vehicle.Speed", "Vehicle.Cabin.Door.Row1.IsOpen"], 10);
        assert_eq!(req.signal_paths.len(), 2);
        assert_eq!(req.buffer_size, 10);
        assert_eq!(req.filter, None);
    }

    #[test]
    fn wire_round_trip_is_lossless() {
        let req = publish_by_path("Vehicle.Speed", float_datapoint(42.0, t0()));
        let back = round_trip(&req).expect("decodes");
        assert_eq!(back, req);
    }

    #[test]
    fn garbage_does_not_decode() {
        assert!(proto::PublishValueRequest::decode(&[0xff, 0xff, 0xff][..]).is_err());
    }
}
