// c36_e04_implement_signal_provider.rs
//
// CONCEPT: Implementing the real `SignalProvider` trait and plugging it in.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section "Implementing a SignalProvider")
// DATABROKER: databroker/src/broker.rs — `trait SignalProvider`, `register_signals`,
//             `get_values_broker`, test doubles such as `StaticValueProvider`
//             and `PartialResponseProvider`;
//             databroker/src/grpc/kuksa_val_v2/val.rs — `impl SignalProvider for Provider`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// A *provider* is whoever owns a signal's value — usually a vehicle ECU
// adapter connected via `OpenProviderStream`. When a client calls
// `GetValue(s)`, the broker asks the provider for a fresh value and falls
// back to the database for anything the provider doesn't answer.
//
// The broker stores providers as `Arc<dyn SignalProvider>`. Trait objects
// can't (yet) have native `async fn`s, so the trait uses
// `#[async_trait::async_trait]`, which rewrites every `async fn` into a
// method returning `Pin<Box<dyn Future + Send>>`. Your impl block must carry
// the same attribute.
//
// YOUR TASK
//   1. `impl SignalProvider for FixedProvider` — the method bodies.
//   2. `register_provider` and `read_values`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use databroker::broker::{
    DataBroker, Datapoint, GetValuesProviderResponse, ReadError, RegisterSignalError,
    SignalProvider,
};
use databroker::permissions::Permissions;
use databroker::types::{DataValue, SignalId, TimeInterval};
use indexmap::IndexMap;
use uuid::Uuid;

/// A provider that answers from a fixed table and counts how often it is asked.
pub struct FixedProvider {
    values: HashMap<i32, DataValue>,
    calls: Arc<AtomicUsize>,
}

impl FixedProvider {
    pub fn new(values: HashMap<i32, DataValue>) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = FixedProvider {
            values,
            calls: Arc::clone(&calls),
        };
        (provider, calls)
    }
}

#[async_trait::async_trait]
impl SignalProvider for FixedProvider {
    /// The broker tells providers how often subscribers want updates. We
    /// don't stream anything, so accept every filter.
    async fn update_filter(
        &self,
        _update_filters: HashMap<SignalId, Option<TimeInterval>>,
    ) -> Result<(), (RegisterSignalError, String)> {
        todo!()
    }

    /// Returning `false` makes housekeeping unregister the provider.
    fn is_available(&self) -> bool {
        todo!()
    }

    /// Count the call, then answer every requested signal we have a value
    /// for (`ts: SystemTime::now()`, `source_ts: None`), in request order.
    /// If we know *none* of them return `Err(())` — the broker then reads
    /// the database instead (the real gRPC provider does the same).
    async fn get_signals_values_from_provider(
        &self,
        signals_ids: Vec<SignalId>,
    ) -> Result<GetValuesProviderResponse, ()> {
        todo!()
    }
}

/// Registers `provider` as the owner of `ids`, using `permissions`.
/// `register_signals` wants a `HashMap<SignalId, TimeInterval>` (use
/// `TimeInterval::new(0)`) and a `Box<dyn SignalProvider + Send + Sync>`.
pub async fn register_provider(
    broker: &DataBroker,
    permissions: &Permissions,
    provider: FixedProvider,
    ids: &[i32],
) -> Result<Uuid, (RegisterSignalError, String)> {
    todo!()
}

/// Reads `ids` through `get_values_broker` (the path `GetValues` takes) and
/// returns `(id, value)` pairs in response order.
pub async fn read_values(
    broker: &DataBroker,
    permissions: &Permissions,
    ids: &[i32],
) -> Result<Vec<(i32, DataValue)>, (ReadError, i32)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use databroker::broker::EntryUpdate;
    use databroker::permissions::{Permission, ALLOW_ALL};
    use databroker::types::{ChangeType, DataType, EntryType};

    async fn setup() -> (DataBroker, i32, i32, i32) {
        let broker = DataBroker::default();
        let access = broker.authorized_access(&ALLOW_ALL);
        let mut ids = Vec::new();
        for path in ["Vehicle.Speed", "Vehicle.Engine.Speed", "Vehicle.Odometer"] {
            ids.push(
                access
                    .add_entry(
                        path.into(),
                        DataType::Float,
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
        // The database knows an odometer and an engine speed value.
        for (id, value) in [(ids[2], 1000.0), (ids[1], 800.0)] {
            let update = EntryUpdate {
                datapoint: Some(Datapoint {
                    ts: SystemTime::now(),
                    source_ts: None,
                    value: DataValue::Float(value),
                }),
                ..Default::default()
            };
            access.update_entries([(id, update)]).await.unwrap();
        }
        (broker, ids[0], ids[1], ids[2])
    }

    #[tokio::test]
    async fn provider_values_win_and_the_rest_comes_from_the_database() {
        let (broker, speed, _, odometer) = setup().await;
        let (provider, calls) =
            FixedProvider::new(HashMap::from([(speed, DataValue::Float(88.0))]));
        register_provider(&broker, &ALLOW_ALL, provider, &[speed])
            .await
            .expect("registration succeeds");

        // Request order is preserved: odometer first, then speed.
        let values = read_values(&broker, &ALLOW_ALL, &[odometer, speed])
            .await
            .unwrap();
        assert_eq!(
            values,
            vec![
                (odometer, DataValue::Float(1000.0)),
                (speed, DataValue::Float(88.0))
            ]
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn partial_answers_fall_back_per_signal() {
        let (broker, speed, rpm, _) = setup().await;
        // Registered for speed AND rpm, but only knows speed.
        let (provider, _) = FixedProvider::new(HashMap::from([(speed, DataValue::Float(5.0))]));
        register_provider(&broker, &ALLOW_ALL, provider, &[speed, rpm])
            .await
            .unwrap();
        let values = read_values(&broker, &ALLOW_ALL, &[speed, rpm])
            .await
            .unwrap();
        assert_eq!(
            values,
            vec![
                (speed, DataValue::Float(5.0)),
                (rpm, DataValue::Float(800.0))
            ]
        );
    }

    #[tokio::test]
    async fn a_provider_that_knows_nothing_falls_back_entirely() {
        let (broker, _, rpm, _) = setup().await;
        let (provider, calls) = FixedProvider::new(HashMap::new());
        register_provider(&broker, &ALLOW_ALL, provider, &[rpm])
            .await
            .unwrap();
        assert_eq!(
            read_values(&broker, &ALLOW_ALL, &[rpm]).await.unwrap(),
            vec![(rpm, DataValue::Float(800.0))]
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn one_owner_per_signal() {
        let (broker, speed, _, _) = setup().await;
        let (first, _) = FixedProvider::new(HashMap::new());
        let (second, _) = FixedProvider::new(HashMap::new());
        register_provider(&broker, &ALLOW_ALL, first, &[speed])
            .await
            .unwrap();
        let err = register_provider(&broker, &ALLOW_ALL, second, &[speed])
            .await
            .unwrap_err();
        assert!(matches!(
            err.0,
            RegisterSignalError::SignalAlreadyRegistered
        ));
    }

    #[tokio::test]
    async fn providing_needs_the_provide_permission() {
        let (broker, speed, _, _) = setup().await;
        let reader = Permissions::builder()
            .add_read_permission(Permission::All)
            .build()
            .unwrap();
        let (provider, _) = FixedProvider::new(HashMap::new());
        let err = register_provider(&broker, &reader, provider, &[speed])
            .await
            .unwrap_err();
        assert!(matches!(err.0, RegisterSignalError::PermissionDenied));
    }

    #[tokio::test]
    async fn unknown_ids_are_reported() {
        let (broker, _, _, _) = setup().await;
        let err = read_values(&broker, &ALLOW_ALL, &[4711]).await.unwrap_err();
        assert!(matches!(err, (ReadError::NotFound, 4711)));
    }

    #[test]
    fn provider_is_usable_as_a_trait_object() {
        let (provider, _) = FixedProvider::new(HashMap::new());
        // This is how the broker stores providers.
        let shared: Arc<dyn SignalProvider> = Arc::new(provider);
        assert!(shared.is_available());
    }
}
