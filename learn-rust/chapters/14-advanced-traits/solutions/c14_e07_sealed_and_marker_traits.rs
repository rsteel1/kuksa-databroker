// c14_e07_sealed_and_marker_traits.rs
//
// CONCEPT: marker traits (no methods, they just *label* a type), auto traits
//          (`Send`/`Sync`, implemented by the compiler from a type's fields),
//          and the sealed-trait pattern (a public trait outsiders can't implement).
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Marker traits, auto traits and sealed traits")
// DATABROKER: databroker/src/broker.rs — `SignalProvider: Send + Sync + 'static`
//             uses the auto traits as bounds; a provider holding an `Rc`
//             could never be registered.
// EXPECT:  compile error E0277 (`Rc<...>` cannot be sent between threads safely)
//
// The compiler decides `Send`/`Sync` for you: a struct is `Send` iff all its
// fields are. One `Rc` inside makes the whole `Registry` non-Send — and the
// error shows up where you *require* Send, not where you wrote the `Rc`.
//
// YOUR TASK
//   1. Make `Registry` Send + Sync by changing the pointer type it uses
//      (think chapter 12) — in the struct and in `new`.
//   2. `SignalKind` is sealed: it has a private supertrait, so only this
//      module can implement it. Implement it for `Sensor` ("sensor", writable
//      = false) and `Actuator` ("actuator", writable = true) — you'll need to
//      implement the private `Sealed` trait for them too.
//   3. Implement `publish` so it only accepts values of a type marked
//      `Validated` (a marker trait), returning "published <path>".
//   Do not edit the tests.

use std::sync::{Arc, Mutex};

pub mod kinds {
    // Private module => outside code can't name `private::Sealed`, so it
    // can't implement it, so it can't implement `SignalKind` either.
    mod private {
        pub trait Sealed {}
    }

    pub trait SignalKind: private::Sealed {
        const NAME: &'static str;
        const WRITABLE: bool;
    }

    pub struct Sensor;
    pub struct Actuator;

    impl private::Sealed for Sensor {}
    impl private::Sealed for Actuator {}

    impl SignalKind for Sensor {
        const NAME: &'static str = "sensor";
        const WRITABLE: bool = false;
    }

    impl SignalKind for Actuator {
        const NAME: &'static str = "actuator";
        const WRITABLE: bool = true;
    }
}

use kinds::SignalKind;

fn kind_label<K: SignalKind>() -> String {
    format!("{} (writable: {})", K::NAME, K::WRITABLE)
}

/// Marker trait: "this value passed validation". No methods.
trait Validated {}

struct CheckedPath(&'static str);
impl Validated for CheckedPath {}

fn publish<T: Validated>(value: &T, path: &str) -> String {
    let _ = value; // the bound is the whole point; we don't need the value itself
    format!("published {path}")
}

/// Shared list of registered provider names (cloning a Registry shares the list).
#[derive(Clone)]
struct Registry {
    names: Arc<Mutex<Vec<String>>>,
}

impl Registry {
    fn new() -> Self {
        Registry {
            names: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add(&self, name: &str) {
        self.names
            .lock()
            .expect("not poisoned")
            .push(name.to_string());
    }

    fn count(&self) -> usize {
        self.names.lock().expect("not poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn registry_is_thread_safe() {
        assert_send_sync::<Registry>();
        let registry = Registry::new();
        let for_thread = registry.clone();
        std::thread::spawn(move || for_thread.add("gps"))
            .join()
            .expect("thread ran");
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn sealed_kinds() {
        assert_eq!(kind_label::<kinds::Sensor>(), "sensor (writable: false)");
        assert_eq!(kind_label::<kinds::Actuator>(), "actuator (writable: true)");
    }

    #[test]
    fn marker_bound() {
        assert_eq!(
            publish(&CheckedPath("Vehicle.Speed"), "Vehicle.Speed"),
            "published Vehicle.Speed"
        );
    }
}
