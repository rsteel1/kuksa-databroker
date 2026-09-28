// c09_e08_build_provider_chain.rs
//
// CONCEPT: designing with traits and generics: a shared behaviour (`ValueProvider`),
//          several independent implementers, dynamic dispatch to fan out over
//          them, and `From`/`TryFrom` for lossless/fallible conversions.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Static vs dynamic dispatch")
// DATABROKER: databroker/src/broker.rs:198 `trait SignalProvider: Send + Sync + 'static`
//          and the `get_values_broker` fallback-over-providers logic. This
//          exercise is the single-threaded, sync shape of the same idea.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the five `todo!()` bodies below.
//
// A `ValueProvider` is anything that can answer "what's the value at this
// path?". The databroker asks a *chain* of providers in order and uses the
// first answer it gets — exactly what `ProviderChain::read_first` does here.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests or the type/trait signatures.
//
//   1. `impl ValueProvider for StaticProvider`: `read` returns `Some(value)`
//      if `path` equals `self.path`, else `None`.
//   2. `impl ValueProvider for MapProvider`: `read` looks `path` up in
//      `self.values` and clones the match.
//   3. `ProviderChain::register`: box the given provider and push it.
//   4. `ProviderChain::read_first`: return the first `Some` a provider gives,
//      trying providers in registration order. Return `None` if none match.
//   5. `ProviderChain::read_all`: return every match, one per provider that
//      has this path, in registration order (use iterator adaptors, not a
//      hand-written loop with an index).
//   6. `impl From<DataValue> for String`: a human-readable rendering
//      (`Bool(b)` -> `b.to_string()`, `Int(n)` -> `n.to_string()`, `Str(s)` -> `s`).
//   7. `impl TryFrom<DataValue> for i64`: `Ok(n)` for `Int(n)`, else
//      `Err(NotAnInt)`.
//   8. `describe`: a generic helper, `format!("{label}={value}")`.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Bool(bool),
    Int(i64),
    Str(String),
}

trait ValueProvider {
    fn read(&self, path: &str) -> Option<DataValue>;
}

struct StaticProvider {
    path: String,
    value: DataValue,
}

impl ValueProvider for StaticProvider {
    fn read(&self, path: &str) -> Option<DataValue> {
        if path == self.path {
            Some(self.value.clone())
        } else {
            None
        }
    }
}

struct MapProvider {
    values: HashMap<String, DataValue>,
}

impl ValueProvider for MapProvider {
    fn read(&self, path: &str) -> Option<DataValue> {
        self.values.get(path).cloned()
    }
}

struct ProviderChain {
    providers: Vec<Box<dyn ValueProvider>>,
}

impl ProviderChain {
    fn new() -> Self {
        ProviderChain {
            providers: Vec::new(),
        }
    }

    /// Boxes `provider` and adds it to the end of the chain.
    fn register(&mut self, provider: impl ValueProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// The first match, trying providers in registration order.
    fn read_first(&self, path: &str) -> Option<DataValue> {
        // WHY: find_map short-circuits on the first Some, same as the
        // databroker trying providers until one answers.
        self.providers.iter().find_map(|p| p.read(path))
    }

    /// Every match, one per provider that has `path`, in registration order.
    fn read_all(&self, path: &str) -> Vec<DataValue> {
        self.providers.iter().filter_map(|p| p.read(path)).collect()
    }
}

#[derive(Debug, PartialEq)]
struct NotAnInt;

impl From<DataValue> for String {
    fn from(value: DataValue) -> Self {
        match value {
            DataValue::Bool(b) => b.to_string(),
            DataValue::Int(n) => n.to_string(),
            DataValue::Str(s) => s,
        }
    }
}

impl TryFrom<DataValue> for i64 {
    type Error = NotAnInt;

    fn try_from(value: DataValue) -> Result<Self, Self::Error> {
        match value {
            DataValue::Int(n) => Ok(n),
            _ => Err(NotAnInt),
        }
    }
}

/// Renders `"{label}={value}"` for any `Display`-able value.
fn describe<T: std::fmt::Display>(label: &str, value: T) -> String {
    format!("{label}={value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> ProviderChain {
        let mut chain = ProviderChain::new();
        chain.register(StaticProvider {
            path: "Vehicle.Speed".to_string(),
            value: DataValue::Int(42),
        });
        let mut values = HashMap::new();
        values.insert("Vehicle.Speed".to_string(), DataValue::Int(99));
        values.insert("Vehicle.IsMoving".to_string(), DataValue::Bool(true));
        chain.register(MapProvider { values });
        chain
    }

    #[test]
    fn read_first_prefers_earlier_registration() {
        let chain = chain();
        // StaticProvider was registered first and also answers Vehicle.Speed.
        assert_eq!(chain.read_first("Vehicle.Speed"), Some(DataValue::Int(42)));
    }

    #[test]
    fn read_first_falls_back_to_a_later_provider() {
        let chain = chain();
        assert_eq!(
            chain.read_first("Vehicle.IsMoving"),
            Some(DataValue::Bool(true))
        );
    }

    #[test]
    fn read_first_returns_none_when_nobody_has_it() {
        let chain = chain();
        assert_eq!(chain.read_first("Vehicle.Unknown"), None);
    }

    #[test]
    fn read_all_collects_every_matching_provider() {
        let chain = chain();
        assert_eq!(
            chain.read_all("Vehicle.Speed"),
            vec![DataValue::Int(42), DataValue::Int(99)]
        );
        assert_eq!(
            chain.read_all("Vehicle.IsMoving"),
            vec![DataValue::Bool(true)]
        );
        assert_eq!(chain.read_all("Vehicle.Unknown"), Vec::<DataValue>::new());
    }

    #[test]
    fn conversions_round_trip() {
        let s: String = DataValue::Str("hi".to_string()).into();
        assert_eq!(s, "hi");
        let n: i64 = DataValue::Int(7).try_into().unwrap();
        assert_eq!(n, 7);
        assert_eq!(i64::try_from(DataValue::Bool(true)), Err(NotAnInt));
    }

    #[test]
    fn describe_formats_label_and_value() {
        assert_eq!(describe("speed", 42), "speed=42");
        assert_eq!(describe("moving", true), "moving=true");
    }
}
