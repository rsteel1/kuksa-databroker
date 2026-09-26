// c12_e04_arc_shared.rs
//
// CONCEPT: `Arc<T>` — atomically reference-counted shared ownership that
//          works across threads; `Arc<dyn Trait>`; what `Arc::clone` costs.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Arc: shared across threads")
// DATABROKER: databroker/src/broker.rs — `DataBroker` (line 179) is
//          `#[derive(Clone)]` over `Arc<RwLock<Database>>` etc., so cloning
//          the broker into every gRPC task is cheap. Providers are stored as
//          `Arc<dyn SignalProvider>` (line 235) and snapshotted with
//          `provider.signal_provider.clone()` (line 2225).
// EXPECT:  tests fail: `todo!()` panics.
//
// `Arc::clone` never copies the inner value: it atomically increments a
// counter (a few nanoseconds) and returns another pointer to the SAME value.
// That's why "clone everywhere" is fine for `Arc` but a smell for `String`.
//
// YOUR TASK
//   Implement `fan_out` and `available_providers`. Do not edit the tests.

// I AM NOT DONE

use std::sync::Arc;

#[derive(Debug)]
struct Metadata {
    path: String,
    description: String,
}

trait Provider {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
}

struct StaticProvider {
    name: String,
    up: bool,
}

impl Provider for StaticProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_available(&self) -> bool {
        self.up
    }
}

/// Returns `n` handles to the SAME metadata (no deep copies).
fn fan_out(meta: &Arc<Metadata>, n: usize) -> Vec<Arc<Metadata>> {
    todo!()
}

/// Snapshot of the providers that are currently available — new `Arc`
/// handles to the same provider objects, in the same order.
fn available_providers(providers: &[Arc<dyn Provider>]) -> Vec<Arc<dyn Provider>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_out_shares_one_allocation() {
        let meta = Arc::new(Metadata {
            path: "Vehicle.Speed".into(),
            description: "Vehicle speed".into(),
        });
        let handles = fan_out(&meta, 3);
        assert_eq!(handles.len(), 3);
        assert_eq!(Arc::strong_count(&meta), 4);
        assert!(handles.iter().all(|h| Arc::ptr_eq(h, &meta)));
        assert_eq!(handles[0].path, "Vehicle.Speed");
        drop(handles);
        assert_eq!(Arc::strong_count(&meta), 1);
    }

    #[test]
    fn snapshot_of_available_providers() {
        let up: Arc<dyn Provider> = Arc::new(StaticProvider {
            name: "up".into(),
            up: true,
        });
        let down: Arc<dyn Provider> = Arc::new(StaticProvider {
            name: "down".into(),
            up: false,
        });
        let up2: Arc<dyn Provider> = Arc::new(StaticProvider {
            name: "up2".into(),
            up: true,
        });
        let all = vec![up.clone(), down, up2];

        let snapshot = available_providers(&all);
        let names: Vec<&str> = snapshot.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["up", "up2"]);
        assert!(Arc::ptr_eq(&snapshot[0], &up));
        assert_eq!(Arc::strong_count(&up), 3); // `up`, `all[0]`, `snapshot[0]`
    }
}
