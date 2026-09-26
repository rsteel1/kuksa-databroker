// c36_e02_publish_and_subscribe.rs
//
// CONCEPT: Publishing values and consuming the real subscription stream.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section "Publishing and subscribing")
// DATABROKER: databroker/src/broker.rs — `EntryUpdate`, `AuthorizedAccess::update_entries`,
//             `AuthorizedAccess::subscribe`, `EntryUpdates`/`ChangeNotification`;
//             databroker/src/grpc/kuksa_val_v2/val.rs — `publish_value`, `subscribe`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// This is exactly what the gRPC `PublishValue` and `Subscribe` handlers do
// after decoding the request: build an `EntryUpdate`, call
// `update_entries`, and adapt the `Stream<Item = Option<EntryUpdates>>` from
// `subscribe` into protobuf responses. Here you adapt it into plain
// `(path, value)` pairs instead.
//
// YOUR TASK
//   Implement `publish` and `watch`. Do not edit the tests.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::time::SystemTime;

use databroker::broker::{DataBroker, Datapoint, EntryUpdate, EntryUpdates, Field, UpdateError};
use databroker::permissions::{self, Permissions};
use databroker::types::DataValue;
use tokio_stream::{Stream, StreamExt};

/// A boxed stream of `(path, value)` batches (same trick as `SubscribeStream`
/// in val.rs: a boxed trait object has a nameable type).
pub type ValueStream = Pin<Box<dyn Stream<Item = Vec<(String, DataValue)>> + Send>>;

/// Publishes `value` to signal `id` with the caller's `permissions`.
/// Build an `EntryUpdate` that only sets `datapoint`
/// (`ts: SystemTime::now()`, `source_ts: None`) — every other field `None`
/// via `..Default::default()`.
pub async fn publish(
    broker: &DataBroker,
    permissions: &Permissions,
    id: i32,
    value: DataValue,
) -> Result<(), Vec<(i32, UpdateError)>> {
    let update = EntryUpdate {
        datapoint: Some(Datapoint {
            ts: SystemTime::now(),
            source_ts: None,
            value,
        }),
        ..Default::default()
    };
    broker
        .authorized_access(permissions)
        .update_entries([(id, update)])
        .await
}

/// Subscribes (with `ALLOW_ALL`) to the datapoints of `ids`, buffer size 10,
/// no minimum interval. Turn each `Some(EntryUpdates)` into a `Vec` of
/// `(path, value)` pairs sorted by path; drop `None` messages (lag markers).
/// Panic with `expect` if subscribing fails — callers pass valid ids.
pub async fn watch(broker: &DataBroker, ids: &[i32]) -> ValueStream {
    let entries: HashMap<i32, HashSet<Field>> = ids
        .iter()
        .map(|id| (*id, HashSet::from([Field::Datapoint])))
        .collect();
    let stream = broker
        .authorized_access(&permissions::ALLOW_ALL)
        .subscribe(entries, Some(10), None)
        .await
        .expect("subscribing to registered ids succeeds");

    let stream = stream.filter_map(|message: Option<EntryUpdates>| {
        let updates = message?; // WHY: `?` on Option: None (lagged) -> skip
        let mut pairs: Vec<(String, DataValue)> = updates
            .updates
            .into_iter()
            .filter_map(|change| {
                let path = change.update.path?;
                let value = change.update.datapoint?.value;
                Some((path, value))
            })
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Some(pairs)
    });
    Box::pin(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use databroker::types::{ChangeType, DataType, EntryType};
    use std::time::Duration;

    async fn setup() -> (DataBroker, i32, i32) {
        let broker = DataBroker::default();
        let access = broker.authorized_access(&permissions::ALLOW_ALL);
        let speed = access
            .add_entry(
                "Vehicle.Speed".into(),
                DataType::Float,
                ChangeType::OnChange,
                EntryType::Sensor,
                "speed".into(),
                None,
                Some(DataValue::Float(250.0)),
                None,
                None,
            )
            .await
            .unwrap();
        let rpm = access
            .add_entry(
                "Vehicle.Engine.Speed".into(),
                DataType::Uint32,
                ChangeType::OnChange,
                EntryType::Sensor,
                "rpm".into(),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        (broker, speed, rpm)
    }

    #[tokio::test(start_paused = true)]
    async fn initial_snapshot_then_changes() {
        let (broker, speed, rpm) = setup().await;
        let mut stream = watch(&broker, &[speed, rpm]).await;

        assert_eq!(
            stream.next().await.unwrap(),
            vec![
                ("Vehicle.Engine.Speed".to_string(), DataValue::NotAvailable),
                ("Vehicle.Speed".to_string(), DataValue::NotAvailable),
            ]
        );

        publish(
            &broker,
            &permissions::ALLOW_ALL,
            speed,
            DataValue::Float(42.0),
        )
        .await
        .unwrap();
        assert_eq!(
            stream.next().await.unwrap(),
            vec![("Vehicle.Speed".to_string(), DataValue::Float(42.0))]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_values_are_not_broadcast() {
        let (broker, speed, rpm) = setup().await;
        let mut stream = watch(&broker, &[speed]).await;
        stream.next().await; // initial snapshot

        let errors = publish(
            &broker,
            &permissions::ALLOW_ALL,
            speed,
            DataValue::Float(300.0),
        )
        .await
        .unwrap_err();
        assert_eq!(errors, vec![(speed, UpdateError::OutOfBoundsMinMax)]);

        let errors = publish(&broker, &permissions::ALLOW_ALL, rpm, DataValue::Float(1.0))
            .await
            .unwrap_err();
        assert_eq!(errors, vec![(rpm, UpdateError::WrongType)]);

        let nothing = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
        assert!(nothing.is_err(), "no message expected, got {nothing:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn same_value_twice_is_one_notification() {
        let (broker, speed, _) = setup().await;
        let mut stream = watch(&broker, &[speed]).await;
        stream.next().await;
        for _ in 0..2 {
            publish(
                &broker,
                &permissions::ALLOW_ALL,
                speed,
                DataValue::Float(7.0),
            )
            .await
            .unwrap();
        }
        assert!(stream.next().await.is_some());
        let nothing = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
        assert!(nothing.is_err(), "OnChange signals drop repeated values");
    }

    #[tokio::test]
    async fn publishing_needs_the_provide_permission() {
        let (broker, speed, _) = setup().await;
        assert_eq!(
            publish(
                &broker,
                &permissions::ALLOW_NONE,
                speed,
                DataValue::Float(1.0)
            )
            .await
            .unwrap_err(),
            vec![(speed, UpdateError::PermissionDenied)]
        );
    }
}
