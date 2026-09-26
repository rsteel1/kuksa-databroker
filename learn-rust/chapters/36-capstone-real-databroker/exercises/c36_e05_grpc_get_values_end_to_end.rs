// c36_e05_grpc_get_values_end_to_end.rs
//
// CONCEPT: Calling the real gRPC service implementation (`kuksa.val.v2.VAL`)
//          in-process: protobuf messages in, `tonic::Status` codes out.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section "The gRPC layer, in-process")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `impl Val for DataBroker`
//             (`publish_value`, `get_values`, `get_signal`);
//             databroker/src/grpc/server.rs — the `Interceptor` that inserts
//             `Permissions` into request extensions;
//             databroker/src/grpc/kuksa_val_v2/conversions.rs — `to_status_with_code`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// tonic generates a trait (`val_server::Val`) with one async method per RPC;
// the databroker implements it directly on `DataBroker`. Over the network
// the interceptor decodes the JWT and puts a `Permissions` value into the
// request's *extensions*; every handler starts by taking it out again. We
// skip the network and call the trait methods ourselves — exactly what the
// tests at the bottom of val.rs do.
//
// YOUR TASK
//   Implement the four functions. Do not edit the tests.

// I AM NOT DONE

use databroker::broker::DataBroker;
use databroker::permissions::Permissions;
use databroker_proto::kuksa::val::v2 as proto;
use proto::val_server::Val;

/// Wraps `message` in a `tonic::Request` carrying a clone of `permissions`
/// in its extensions — what the auth interceptor does in production.
pub fn authorized<T>(message: T, permissions: &Permissions) -> tonic::Request<T> {
    todo!()
}

/// A `SignalId` addressing a signal by path (the `oneof signal` in the proto).
pub fn by_path(path: &str) -> proto::SignalId {
    todo!()
}

/// Publishes a float through the `PublishValue` RPC. The datapoint needs no
/// timestamp (`None`); the value is `TypedValue::Float`.
pub async fn publish_float(
    broker: &DataBroker,
    permissions: &Permissions,
    path: &str,
    value: f32,
) -> Result<(), tonic::Status> {
    todo!()
}

/// Reads `paths` through the `GetValues` RPC. For every returned datapoint:
/// `Some(f)` if it holds a `TypedValue::Float(f)`, otherwise `None`
/// (a signal that has no value yet comes back with `value: None`).
pub async fn get_floats(
    broker: &DataBroker,
    permissions: &Permissions,
    paths: &[&str],
) -> Result<Vec<Option<f32>>, tonic::Status> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use databroker::permissions::{Permission, ALLOW_ALL};
    use databroker::types::{ChangeType, DataType, DataValue, EntryType};
    use tonic::Code;

    async fn setup() -> DataBroker {
        let broker = DataBroker::default();
        let access = broker.authorized_access(&ALLOW_ALL);
        for path in ["Vehicle.Speed", "Vehicle.Cabin.Temperature"] {
            access
                .add_entry(
                    path.into(),
                    DataType::Float,
                    ChangeType::OnChange,
                    EntryType::Sensor,
                    String::new(),
                    Some(DataValue::Float(-50.0)),
                    Some(DataValue::Float(250.0)),
                    None,
                    None,
                )
                .await
                .unwrap();
        }
        broker
    }

    #[test]
    fn requests_carry_permissions() {
        let request = authorized((), &ALLOW_ALL);
        assert!(request.extensions().get::<Permissions>().is_some());
        assert_eq!(
            by_path("Vehicle.Speed").signal,
            Some(proto::signal_id::Signal::Path("Vehicle.Speed".into()))
        );
    }

    #[tokio::test]
    async fn publish_then_get_in_request_order() {
        let broker = setup().await;
        publish_float(&broker, &ALLOW_ALL, "Vehicle.Speed", 72.5)
            .await
            .unwrap();
        assert_eq!(
            get_floats(
                &broker,
                &ALLOW_ALL,
                &["Vehicle.Cabin.Temperature", "Vehicle.Speed"]
            )
            .await
            .unwrap(),
            vec![None, Some(72.5)]
        );
    }

    #[tokio::test]
    async fn errors_map_to_grpc_status_codes() {
        let broker = setup().await;
        let status = get_floats(&broker, &ALLOW_ALL, &["Vehicle.Nope"])
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::NotFound);

        let status = publish_float(&broker, &ALLOW_ALL, "Vehicle.Speed", 999.0)
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::InvalidArgument);

        let cabin = Permissions::builder()
            .add_read_permission(Permission::Glob("Vehicle.Cabin".into()))
            .build()
            .unwrap();
        let status = get_floats(&broker, &cabin, &["Vehicle.Speed"])
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::PermissionDenied);
        let status = publish_float(&broker, &cabin, "Vehicle.Cabin.Temperature", 20.0)
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::PermissionDenied);
    }

    #[tokio::test]
    async fn no_permissions_means_unauthenticated() {
        let broker = setup().await;
        // A request that never went through the interceptor.
        let request = tonic::Request::new(proto::GetValuesRequest {
            signal_ids: vec![by_path("Vehicle.Speed")],
        });
        let status = broker.get_values(request).await.unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
    }
}
