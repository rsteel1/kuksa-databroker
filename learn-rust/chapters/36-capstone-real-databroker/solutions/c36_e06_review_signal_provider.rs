// c36_e06_review_signal_provider.rs
//
// CONCEPT: code review — this file COMPILES against the REAL `databroker`
//          crate, but has 3 planted bugs typical of a hand-written
//          `SignalProvider`: availability tied to "have I ever answered"
//          instead of "am I up", a fallback rule inverted from "we know
//          none of them" to "we don't know ALL of them", and a helper that
//          "helpfully" sorts ids before asking the broker, throwing away
//          the one guarantee `GetValues` makes.
// GUIDE:   chapters/36-capstone-real-databroker/guide.md  (section
//          "Implementing a SignalProvider")
// DATABROKER: databroker/src/broker.rs:198-208 — `trait SignalProvider`;
//             `is_available` gates whether the broker calls the provider at
//             all (a provider that's actually down should say so, not
//             report "unavailable" just because it has answered before);
//             `get_signals_values_from_provider` returns `Err(())` only
//             when the provider can answer NONE of the requested ids —
//             databroker/src/broker.rs:2293-2302 then falls back to the
//             database for every id in that response, not just the unknown
//             ones; databroker/src/broker.rs:2318-2325 — the FINAL response
//             is built by walking `vss_signals` (the caller's own list,
//             in the caller's own order) — "the gRPC contract promises
//             that data points are returned in the same order as the
//             requested signals."
// EXPECT:  tests fail: a provider that has already answered once starts
//          reporting itself unavailable, a provider that knows only SOME of
//          the requested signals is treated as knowing NONE of them, and
//          results come back sorted by id instead of in request order.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests.

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
    async fn update_filter(
        &self,
        _update_filters: HashMap<SignalId, Option<TimeInterval>>,
    ) -> Result<(), (RegisterSignalError, String)> {
        Ok(())
    }

    /// Meant to mean "is this provider up at all". A provider that has
    /// already answered a request once is not thereby "down".
    fn is_available(&self) -> bool {
        // BUG 1: `is_available` tracked "have I ever answered a request"
        // instead of "am I up". This fixed-table provider never goes down.
        true
    }

    /// Answers every requested signal we have a value for. Only errors out
    /// if we can't answer ANY of them — the broker then reads the database
    /// for the rest.
    async fn get_signals_values_from_provider(
        &self,
        signals_ids: Vec<SignalId>,
    ) -> Result<GetValuesProviderResponse, ()> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        // BUG 2: `.any(...)` errors out as soon as a SINGLE requested id is
        // unknown, even if every other one is known — the broker then
        // throws away every answer this provider actually had and falls
        // back to the database for all of them. The contract is "error
        // only if we know NONE of them": `.all(...)`.
        if signals_ids
            .iter()
            .all(|id| !self.values.contains_key(&id.id()))
        {
            return Err(());
        }
        let mut entries = IndexMap::new();
        for id in signals_ids {
            if let Some(value) = self.values.get(&id.id()) {
                entries.insert(
                    id,
                    Datapoint {
                        ts: SystemTime::now(),
                        source_ts: None,
                        value: value.clone(),
                    },
                );
            }
        }
        Ok(GetValuesProviderResponse { entries })
    }
}

/// Reads `ids` through `get_values_broker` (the path `GetValues` takes) and
/// returns `(id, value)` pairs. `get_values_broker` already answers in
/// request order, so no re-sorting is needed (or wanted) here.
pub async fn read_values(
    broker: &DataBroker,
    permissions: &Permissions,
    ids: &[i32],
) -> Result<Vec<(i32, DataValue)>, (ReadError, i32)> {
    // BUG 3: sorting the ids before calling `get_values_broker` changes the
    // order the RESPONSE comes back in too, because `get_values_broker`
    // preserves whatever order it was given — it has no way to recover the
    // caller's original request order afterwards.
    let signal_ids: Vec<SignalId> = ids.iter().copied().map(SignalId::new).collect();
    let access = broker.authorized_access(permissions);
    let response = access.get_values_broker(signal_ids).await?;
    Ok(response
        .entries
        .into_iter()
        .map(|(id, dp)| (id.id(), dp.value))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use databroker::broker::EntryUpdate;
    use databroker::permissions::ALLOW_ALL;
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
    async fn a_provider_stays_available_after_answering() {
        let (provider, _) = FixedProvider::new(HashMap::new());
        let shared: Arc<dyn SignalProvider> = Arc::new(provider);
        assert!(shared.is_available());
        let _ = shared.get_signals_values_from_provider(vec![]).await;
        assert!(
            shared.is_available(),
            "answering a request must not make the provider report itself unavailable"
        );
    }

    #[tokio::test]
    async fn partial_answers_fall_back_per_signal() {
        let (broker, speed, rpm, _) = setup().await;
        // Knows speed but not rpm: must still answer speed, and let the
        // broker fall back to the database for rpm.
        let (provider, _) = FixedProvider::new(HashMap::from([(speed, DataValue::Float(5.0))]));
        broker
            .authorized_access(&ALLOW_ALL)
            .register_signals(
                HashMap::from([
                    (SignalId::new(speed), TimeInterval::new(0)),
                    (SignalId::new(rpm), TimeInterval::new(0)),
                ]),
                Box::new(provider),
            )
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
    async fn results_preserve_request_order() {
        let (broker, speed, _, odometer) = setup().await;
        // Request order is odometer first, then speed — the opposite of id order.
        let values = read_values(&broker, &ALLOW_ALL, &[odometer, speed])
            .await
            .unwrap();
        assert_eq!(
            values,
            vec![
                (odometer, DataValue::Float(1000.0)),
                (speed, DataValue::NotAvailable)
            ],
            "results must come back in request order, not sorted by id"
        );
    }
}
