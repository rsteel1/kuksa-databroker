// c33_e03_api_guidelines.rs
//
// CONCEPT: designing a public API the way the Rust API Guidelines describe:
//          conversion naming (`as_`/`to_`/`into_`), no `get_` prefix,
//          `Option`/`Result` instead of sentinels and bools, flexible
//          parameters (`impl Into<String>`, `&str`), iterators instead of
//          allocated `Vec`s, `#[must_use]`, builders, `Debug` everywhere.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section "Designing APIs other people enjoy")
// DATABROKER: databroker/src/broker.rs — `get_datapoint`, `get_metadata`,
//             `get_entry_by_path` (`get_` prefixes), and `add_entry` with nine
//             positional parameters (`#[allow(clippy::too_many_arguments)]`).
// EXPECT:  compile errors, because the tests use the NEW API: E0599 (no function
//          `new`/`builder` on `Store`), E0425/E0433 (`SetError`, `StoreBuilder`
//          not found), E0277 (`Store` doesn't implement `Debug`)
//
// The code below works, but it's awkward to use and easy to misuse: callers
// get `-1.0` for "missing", a `bool` they can silently ignore, a fresh `Vec`
// on every call, and a `&String` they have to allocate for.
//
// YOUR TASK
//   Rewrite `Store` so the tests compile and pass. The tests are the spec:
//   read them first and list every API decision they imply. You may delete
//   and rename anything above the tests. Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::fmt;

// Works, but every line of this API is a small trap for its callers.
pub struct Store {
    pub map: HashMap<String, f64>,
    pub last_error: Option<String>,
}

impl Store {
    pub fn create() -> Store {
        Store {
            map: HashMap::new(),
            last_error: None,
        }
    }

    /// Returns -1.0 if the signal doesn't exist.
    pub fn get_value(&self, path: &String) -> f64 {
        match self.map.get(path) {
            Some(v) => *v,
            None => -1.0,
        }
    }

    /// Returns false (and sets `last_error`) if the value was rejected.
    pub fn set_value(&mut self, path: String, value: f64) -> bool {
        if path.is_empty() || !value.is_finite() {
            self.last_error = Some("bad input".to_string());
            return false;
        }
        self.map.insert(path, value);
        true
    }

    /// Allocates a fresh Vec (and clones every key) on each call.
    pub fn get_paths(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    pub fn get_count(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_debug<T: fmt::Debug>() {}
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

    #[test]
    fn value_returns_an_option_and_takes_str() {
        let mut store = Store::new();
        assert_eq!(store.value("Vehicle.Speed"), None);
        store.set("Vehicle.Speed", 42.0).expect("valid set");
        let speed: Option<f64> = store.value("Vehicle.Speed");
        assert_eq!(speed, Some(42.0));
    }

    #[test]
    fn set_accepts_str_and_string_and_returns_the_previous_value() {
        let mut store = Store::default();
        assert_eq!(store.set("A", 1.0), Ok(None));
        assert_eq!(store.set(String::from("A"), 2.0), Ok(Some(1.0)));
    }

    #[test]
    fn set_reports_why_it_failed() {
        let mut store = Store::new();
        assert_eq!(store.set("", 1.0), Err(SetError::EmptyPath));
        assert_eq!(store.set("A", f64::NAN), Err(SetError::NotFinite));
        assert_eq!(
            store.set("A", 1.0).map_err(|e| e.to_string()),
            Ok(None),
            "errors implement Display"
        );
        assert_error::<SetError>();
    }

    #[test]
    fn paths_is_a_borrowing_iterator() {
        let mut store = Store::new();
        store.set("B", 1.0).expect("valid");
        store.set("A", 1.0).expect("valid");
        let mut paths: Vec<&str> = store.paths().collect();
        paths.sort_unstable();
        assert_eq!(paths, ["A", "B"]);
        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
        assert!(Store::new().is_empty());
    }

    #[test]
    fn conversion_methods_follow_as_to_into() {
        let mut store = Store::new();
        store.set("B", 2.0).expect("valid");
        store.set("A", 1.0).expect("valid");

        let view: &HashMap<String, f64> = store.as_map();
        assert_eq!(view.len(), 2);

        assert_eq!(store.to_sorted_lines(), ["A=1", "B=2"]);
        // `to_` borrowed; the store is still usable:
        assert_eq!(store.value("A"), Some(1.0));

        let pairs: Vec<(String, f64)> = store.into_sorted_pairs();
        assert_eq!(pairs, [("A".to_string(), 1.0), ("B".to_string(), 2.0)]);
        // `store` has been moved: using it here would be E0382.
    }

    #[test]
    fn builder_configures_read_only_prefixes() {
        let mut store = Store::builder()
            .capacity(16)
            .read_only_prefix("Vehicle.VehicleIdentification")
            .build();
        assert_eq!(
            store.set("Vehicle.VehicleIdentification.VIN", 1.0),
            Err(SetError::ReadOnly {
                path: "Vehicle.VehicleIdentification.VIN".to_string()
            })
        );
        assert_eq!(store.set("Vehicle.Speed", 1.0), Ok(None));
        assert!(store.as_map().capacity() >= 16);
    }

    #[test]
    fn public_types_implement_debug() {
        assert_debug::<Store>();
        assert_debug::<StoreBuilder>();
        assert_debug::<SetError>();
    }
}
