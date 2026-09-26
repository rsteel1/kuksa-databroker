// c36_e03_permissions_denied.rs
//
// CONCEPT: Building real `Permissions` (builder pattern + VSS globs) and
//          seeing every access path enforce them.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section "Permissions end to end")
// DATABROKER: databroker/src/permissions.rs — `PermissionBuilder`, `Permission::Glob`,
//             `can_read`; databroker/src/authorization/jwt/decoder.rs —
//             `TryFrom<Claims> for Permissions` (scopes -> builder calls);
//             databroker/src/broker.rs — `EntryReadAccess`, `filter_map_entries`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// In production, `Permissions` come from a JWT: the token's `scope` claim
// (e.g. "read:Vehicle.Cabin provide:Vehicle.Speed") is parsed and fed into
// `Permissions::builder()`. Here you call the builder yourself.
//
// YOUR TASK
//   Implement the four functions. Do not edit the tests.

use std::time::{Duration, SystemTime};

use databroker::broker::{DataBroker, EntryReadAccess};
use databroker::permissions::{Permission, Permissions};

/// May read everything under the "Vehicle.Cabin" branch (a glob without `*`
/// matches the path itself and everything below it).
pub fn cabin_reader() -> Permissions {
    Permissions::builder()
        .add_read_permission(Permission::Glob("Vehicle.Cabin".to_string()))
        .build()
        .expect("static glob is valid")
}

/// May *provide* (publish values for) "Vehicle.Speed" only.
/// Note that reading it is implied by the provide permission.
pub fn speed_provider() -> Permissions {
    Permissions::builder()
        .add_provide_permission(Permission::Glob("Vehicle.Speed".to_string()))
        .build()
        .expect("static glob is valid")
}

/// May read everything (`Permission::All`), but the token expired an hour ago.
pub fn expired_reader() -> Permissions {
    Permissions::builder()
        .add_read_permission(Permission::All)
        .expires_at(SystemTime::now() - Duration::from_secs(3600))
        .build()
        .expect("no globs to compile")
}

/// Paths whose *value* this caller may read, sorted. Use
/// `filter_map_entries`: `EntryReadAccess::datapoint()` returns `Err` when
/// the read permission check failed, while `metadata()` always works.
pub async fn readable_paths(broker: &DataBroker, permissions: &Permissions) -> Vec<String> {
    let mut paths = broker
        .authorized_access(permissions)
        .filter_map_entries(|entry: EntryReadAccess| {
            entry.datapoint().ok()?;
            Some(entry.metadata().path.clone())
        })
        .await;
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use databroker::broker::{Datapoint, EntryUpdate, ReadError, UpdateError};
    use databroker::permissions::ALLOW_ALL;
    use databroker::types::{ChangeType, DataType, DataValue, EntryType};

    async fn setup() -> (DataBroker, i32, i32) {
        let broker = DataBroker::default();
        let access = broker.authorized_access(&ALLOW_ALL);
        let mut ids = Vec::new();
        for (path, data_type) in [
            ("Vehicle.Speed", DataType::Float),
            ("Vehicle.Cabin.Door.Row1.IsOpen", DataType::Bool),
        ] {
            ids.push(
                access
                    .add_entry(
                        path.into(),
                        data_type,
                        ChangeType::OnChange,
                        EntryType::Sensor,
                        String::new(),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
                    .unwrap(),
            );
        }
        (broker, ids[0], ids[1])
    }

    fn set(value: DataValue) -> EntryUpdate {
        EntryUpdate {
            datapoint: Some(Datapoint {
                ts: SystemTime::now(),
                source_ts: None,
                value,
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn cabin_reader_sees_only_the_cabin() {
        let (broker, speed, door) = setup().await;
        let perms = cabin_reader();
        let access = broker.authorized_access(&perms);
        assert!(access.get_datapoint(door).await.is_ok());
        assert!(matches!(
            access.get_datapoint(speed).await,
            Err(ReadError::PermissionDenied)
        ));
        assert_eq!(
            readable_paths(&broker, &perms).await,
            vec!["Vehicle.Cabin.Door.Row1.IsOpen"]
        );
        // Reading is not writing.
        assert_eq!(
            access
                .update_entries([(door, set(DataValue::Bool(true)))])
                .await
                .unwrap_err(),
            vec![(door, UpdateError::PermissionDenied)]
        );
    }

    #[tokio::test]
    async fn speed_provider_can_publish_and_read_speed_only() {
        let (broker, speed, door) = setup().await;
        let perms = speed_provider();
        let access = broker.authorized_access(&perms);
        access
            .update_entries([(speed, set(DataValue::Float(12.5)))])
            .await
            .expect("provide permission allows publishing");
        assert_eq!(
            access.get_datapoint(speed).await.unwrap().value,
            DataValue::Float(12.5)
        );
        assert_eq!(
            access
                .update_entries([(door, set(DataValue::Bool(true)))])
                .await
                .unwrap_err(),
            vec![(door, UpdateError::PermissionDenied)]
        );
        assert_eq!(readable_paths(&broker, &perms).await, vec!["Vehicle.Speed"]);
    }

    #[tokio::test]
    async fn expired_tokens_are_rejected_everywhere() {
        let (broker, speed, _) = setup().await;
        let perms = expired_reader();
        assert!(perms.is_expired());
        let access = broker.authorized_access(&perms);
        assert!(matches!(
            access.get_datapoint(speed).await,
            Err(ReadError::PermissionExpired)
        ));
        assert!(readable_paths(&broker, &perms).await.is_empty());
    }

    #[tokio::test]
    async fn allow_all_reads_everything() {
        let (broker, _, _) = setup().await;
        assert_eq!(readable_paths(&broker, &ALLOW_ALL).await.len(), 2);
    }
}
