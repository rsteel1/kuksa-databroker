// c08_e05_build_signal_store_module.rs
//
// CONCEPT: writing methods behind a small module boundary, then
//          re-exporting the public API with `pub use`.
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Paths, use and re-exports")
// DATABROKER: databroker/src/broker.rs lines 16-18 — `pub use crate::types;
//             pub use crate::types::{DataType, DataValue, ...};` reshapes
//             the public API independently of where types actually live,
//             exactly like the `pub use signals::...` lines below.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. `mod signals` and the two `pub use` re-exports are already
// wired up; implement the three methods and the free function.
//
// YOUR TASK
//   Implement `Store::record`, `Store::snapshot`, `Store::latest` and
//   `debug_dump`.
//   Do not edit the tests, and do not change any signature or visibility.

mod signals {
    pub mod types {
        #[derive(Debug, Clone, PartialEq)]
        pub struct Reading {
            pub path: String,
            pub value: f64,
        }
    }

    use types::Reading;

    #[derive(Default)]
    pub struct Store {
        entries: Vec<Reading>,
    }

    impl Store {
        /// Records a reading. Rejects (with an error message) any path
        /// that doesn't start with `"Vehicle."`.
        pub fn record(&mut self, path: &str, value: f64) -> Result<(), String> {
            if !path.starts_with("Vehicle.") {
                return Err(format!("invalid path: {path}"));
            }
            self.entries.push(Reading {
                path: path.to_string(),
                value,
            });
            Ok(())
        }

        /// Every reading recorded so far, in insertion order.
        pub fn snapshot(&self) -> &[Reading] {
            &self.entries
        }

        /// The most recently recorded reading for `path`, if any (a path
        /// can be recorded more than once; the newest one wins).
        pub fn latest(&self, path: &str) -> Option<&Reading> {
            self.entries.iter().rev().find(|r| r.path == path)
        }
    }
}

// Re-exports: callers write `Reading` and `Store`, never
// `signals::types::Reading` or `signals::Store`.
pub use signals::types::Reading;
pub use signals::Store;

/// A "path=value" dump, one reading per line, sorted by path.
/// `pub(crate)`: nothing outside this crate needs it.
pub(crate) fn debug_dump(store: &Store) -> String {
    let mut entries: Vec<&Reading> = store.snapshot().iter().collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = String::new();
    for reading in entries {
        out.push_str(&format!("{}={}\n", reading.path, reading.value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_rejects_non_vehicle_paths() {
        let mut store = Store::default();
        assert!(store.record("Engine.RPM", 1.0).is_err());
        assert!(store.snapshot().is_empty());
    }

    #[test]
    fn record_accepts_and_keeps_order() {
        let mut store = Store::default();
        store.record("Vehicle.Speed", 10.0).unwrap();
        store.record("Vehicle.RPM", 3000.0).unwrap();
        assert_eq!(
            store.snapshot().to_vec(),
            vec![
                Reading {
                    path: "Vehicle.Speed".to_string(),
                    value: 10.0
                },
                Reading {
                    path: "Vehicle.RPM".to_string(),
                    value: 3000.0
                },
            ]
        );
    }

    #[test]
    fn latest_finds_the_most_recent_matching_reading() {
        let mut store = Store::default();
        store.record("Vehicle.Speed", 10.0).unwrap();
        store.record("Vehicle.RPM", 3000.0).unwrap();
        store.record("Vehicle.Speed", 20.0).unwrap();
        assert_eq!(store.latest("Vehicle.Speed").unwrap().value, 20.0);
        assert_eq!(store.latest("Vehicle.Fuel"), None);
    }

    #[test]
    fn debug_dump_is_sorted_by_path() {
        let mut store = Store::default();
        store.record("Vehicle.Speed", 10.0).unwrap();
        store.record("Vehicle.RPM", 3000.0).unwrap();
        assert_eq!(debug_dump(&store), "Vehicle.RPM=3000\nVehicle.Speed=10\n");
    }
}
