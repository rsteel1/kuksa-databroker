// c30_e06_call_val_service_directly.rs
//
// CONCEPT: a tonic service is just a trait — call the real databroker's `Val` impl without a network.
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "The Val trait: a service is a trait")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `impl proto::val_server::Val for broker::DataBroker`
//             (`get_value`, `publish_value`) and its tests (e.g. `test_get_value_id_ok`)
// EXPECT:  tests fail: `todo!()` panics
//
// tonic generates a trait `val_server::Val` from `service VAL { ... }` in
// val.proto: one async method per rpc. The databroker implements it on
// `DataBroker`. The generated `ValServer<T: Val>` wrapper does the HTTP/2 +
// protobuf work and calls these methods. So in tests (and here) we can skip
// the network and call the methods directly — exactly how val.rs tests work.
//
// Remember: without the interceptor, nobody attaches `Permissions` to the
// request. Handlers answer `Unauthenticated` if they're missing, so we insert
// them into the extensions ourselves.
//
// YOUR TASK
//   Implement `request_with`, `publish_speed` and `read_speed` using the REAL
//   databroker. Bring the `Val` trait into scope to call its methods.
//   Do not edit `setup` or the tests.

use databroker::broker::{ChangeType, DataBroker, DataType, EntryType};
use databroker::permissions::{self, Permission, Permissions};
use databroker_proto::kuksa::val::v2 as proto;
use databroker_proto::kuksa::val::v2::val_server::Val;
use tonic::{Request, Status};

const SPEED: &str = "Vehicle.Speed";

/// Registers Vehicle.Speed (float sensor, km/h). Returns its id.
async fn setup() -> (DataBroker, i32) {
    let broker = DataBroker::default();
    let id = broker
        .authorized_access(&permissions::ALLOW_ALL)
        .add_entry(
            SPEED.to_string(),
            DataType::Float,
            ChangeType::OnChange,
            EntryType::Sensor,
            "Vehicle speed".to_string(),
            None,
            None,
            None,
            Some("km/h".to_string()),
        )
        .await
        .expect("registration succeeds");
    (broker, id)
}

/// Wrap a message in a tonic::Request carrying `permissions` in its extensions.
fn request_with<T>(message: T, permissions: &Permissions) -> Request<T> {
    let mut request = Request::new(message);
    request.extensions_mut().insert(permissions.clone());
    request
}

/// Publish a float value to Vehicle.Speed (addressed by path) via `Val::publish_value`.
async fn publish_speed(
    broker: &DataBroker,
    kmh: f32,
    permissions: &Permissions,
) -> Result<(), Status> {
    let message = proto::PublishValueRequest {
        signal_id: Some(proto::SignalId {
            signal: Some(proto::signal_id::Signal::Path(SPEED.to_string())),
        }),
        data_point: Some(proto::Datapoint {
            timestamp: None,
            value: Some(proto::Value {
                typed_value: Some(proto::value::TypedValue::Float(kmh)),
            }),
        }),
    };
    broker
        .publish_value(request_with(message, permissions))
        .await
        .map(|_response| ())
}

/// Read Vehicle.Speed by id via `Val::get_value`. `Ok(None)` when the signal
/// exists but has no value yet (the datapoint's `value` is `None`).
async fn read_speed(
    broker: &DataBroker,
    id: i32,
    permissions: &Permissions,
) -> Result<Option<f32>, Status> {
    let message = proto::GetValueRequest {
        signal_id: Some(proto::SignalId {
            signal: Some(proto::signal_id::Signal::Id(id)),
        }),
    };
    let response = broker
        .get_value(request_with(message, permissions))
        .await?
        .into_inner();
    let speed = response
        .data_point
        .and_then(|dp| dp.value)
        .and_then(|value| match value.typed_value {
            Some(proto::value::TypedValue::Float(kmh)) => Some(kmh),
            _ => None,
        });
    Ok(speed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn read_only_speed() -> Permissions {
        Permissions::builder()
            .add_read_permission(Permission::Glob(SPEED.to_string()))
            .build()
            .expect("valid glob")
    }

    #[tokio::test]
    async fn no_value_yet() {
        let (broker, id) = setup().await;
        assert_eq!(
            read_speed(&broker, id, &permissions::ALLOW_ALL)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn publish_then_read() {
        let (broker, id) = setup().await;
        publish_speed(&broker, 88.5, &permissions::ALLOW_ALL)
            .await
            .expect("publish ok");
        assert_eq!(
            read_speed(&broker, id, &permissions::ALLOW_ALL)
                .await
                .unwrap(),
            Some(88.5)
        );
    }

    #[tokio::test]
    async fn read_only_token_cannot_publish() {
        let (broker, id) = setup().await;
        let status = publish_speed(&broker, 10.0, &read_only_speed())
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::PermissionDenied);
        // ...but reading is fine.
        assert!(read_speed(&broker, id, &read_only_speed()).await.is_ok());
    }

    #[tokio::test]
    async fn missing_permissions_means_unauthenticated() {
        let (broker, id) = setup().await;
        let bare = Request::new(proto::GetValueRequest {
            signal_id: Some(proto::SignalId {
                signal: Some(proto::signal_id::Signal::Id(id)),
            }),
        });
        let status = broker.get_value(bare).await.unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
    }

    #[tokio::test]
    async fn unknown_id_is_not_found() {
        let (broker, id) = setup().await;
        let status = read_speed(&broker, id + 1000, &permissions::ALLOW_ALL)
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::NotFound);
    }
}
