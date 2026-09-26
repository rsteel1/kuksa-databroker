// c36_e01_register_and_read.rs
//
// CONCEPT: Using the real `databroker` crate as a library: register and read.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section "Registering and reading signals")
// DATABROKER: databroker/src/broker.rs — `DataBroker::default`, `DataBroker::authorized_access`,
//             `AuthorizedAccess::add_entry`, `get_datapoint_by_path`, `map_entries`;
//             databroker/src/main.rs — `add_kuksa_entry` (how the binary registers signals)
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// From here on you drive the *real* broker. Everything you built in chapter
// 22 has a counterpart with (almost) the same name — now with all 20+ data
// types, actuators, providers and OpenTelemetry hooks.
//
// `permissions::ALLOW_ALL` is a `lazy_static` global; `&permissions::ALLOW_ALL`
// auto-derefs to `&Permissions`. That's what the binary uses when
// authorization is disabled.
//
// YOUR TASK
//   Implement the four functions below with the real API. Read
//   `AuthorizedAccess::add_entry` in broker.rs for the argument order.
//   Do not edit the tests.

// I AM NOT DONE

use databroker::broker::{DataBroker, EntryReadAccess, ReadError, RegistrationError};
use databroker::permissions;
use databroker::types::{ChangeType, DataType, DataValue, EntryType};

/// Registers "Vehicle.Speed": Float, OnChange, Sensor, description
/// "Vehicle speed", min Float(0.0), max Float(250.0), no allowed values,
/// unit "km/h". Use `permissions::ALLOW_ALL`.
pub async fn register_speed(broker: &DataBroker) -> Result<i32, RegistrationError> {
    todo!()
}

/// Registers "Vehicle.Powertrain.Transmission.Mode": String, OnChange,
/// Actuator, description "Gear mode", no min/max, allowed values
/// `DataValue::StringArray(["P", "R", "N", "D"])`, no unit.
/// (In the real broker `allowed` is a single *array* value.)
pub async fn register_mode(broker: &DataBroker) -> Result<i32, RegistrationError> {
    todo!()
}

/// The current value of `path` (not the whole `Datapoint`).
pub async fn current_value(broker: &DataBroker, path: &str) -> Result<DataValue, ReadError> {
    todo!("get_datapoint_by_path, then keep only .value")
}

/// Every registered path, sorted. Use `map_entries`, whose closure receives
/// an `EntryReadAccess` (an entry *plus* the result of the read-permission
/// check; `metadata()` is always available).
pub async fn sorted_paths(broker: &DataBroker) -> Vec<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registers_and_reads_back_metadata() {
        let broker = DataBroker::default();
        let speed = register_speed(&broker).await.expect("valid registration");
        let entry = broker
            .authorized_access(&permissions::ALLOW_ALL)
            .get_entry_by_id(speed)
            .await
            .expect("registered");
        assert_eq!(entry.metadata.path, "Vehicle.Speed");
        assert_eq!(entry.metadata.data_type, DataType::Float);
        assert_eq!(entry.metadata.entry_type, EntryType::Sensor);
        assert_eq!(entry.metadata.min, Some(DataValue::Float(0.0)));
        assert_eq!(entry.metadata.max, Some(DataValue::Float(250.0)));
        assert_eq!(entry.metadata.unit.as_deref(), Some("km/h"));
        assert_eq!(entry.metadata.description, "Vehicle speed");
    }

    #[tokio::test]
    async fn registering_twice_returns_the_same_id() {
        let broker = DataBroker::default();
        let first = register_speed(&broker).await.unwrap();
        let second = register_speed(&broker).await.unwrap();
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn new_signals_are_not_available() {
        let broker = DataBroker::default();
        register_speed(&broker).await.unwrap();
        assert_eq!(
            current_value(&broker, "Vehicle.Speed").await.unwrap(),
            DataValue::NotAvailable
        );
        assert!(matches!(
            current_value(&broker, "Vehicle.Nope").await,
            Err(ReadError::NotFound)
        ));
    }

    #[tokio::test]
    async fn allowed_values_are_stored() {
        let broker = DataBroker::default();
        let mode = register_mode(&broker).await.unwrap();
        let metadata = broker
            .authorized_access(&permissions::ALLOW_ALL)
            .get_metadata(mode)
            .await
            .unwrap();
        assert_eq!(metadata.entry_type, EntryType::Actuator);
        assert_eq!(
            metadata.allowed,
            Some(DataValue::StringArray(vec![
                "P".into(),
                "R".into(),
                "N".into(),
                "D".into()
            ]))
        );
    }

    #[tokio::test]
    async fn lists_paths_in_order() {
        let broker = DataBroker::default();
        register_speed(&broker).await.unwrap();
        register_mode(&broker).await.unwrap();
        assert_eq!(
            sorted_paths(&broker).await,
            vec!["Vehicle.Powertrain.Transmission.Mode", "Vehicle.Speed"]
        );
    }
}
