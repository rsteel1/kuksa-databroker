// c09_e02_default_methods.rs
//
// CONCEPT: default method implementations; overriding them; calling other
//          trait methods from a default.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Default methods")
// DATABROKER: databroker/src/broker.rs — `SignalProvider` has no defaults, so
//             every test double (HangingSignalProvider, CountingSignalProvider,
//             ...) repeats `fn is_available(&self) -> bool { true }`. A
//             default method would remove that repetition.
// EXPECT:  tests fail: `todo!()` panics
//
// A trait method can have a body. Implementors get it for free and may
// override it. Defaults can call the trait's *required* methods, which is
// how one small required method can power several convenient ones.
//
// YOUR TASK
//   1. Write the default bodies of `is_available` (always true) and
//      `status_line`.
//   2. `Disconnected` must override `is_available` to return false.
//   Do not edit the tests.

trait Provider {
    /// Required: every provider has a name.
    fn name(&self) -> String;

    /// Default: providers are available unless they say otherwise.
    fn is_available(&self) -> bool {
        true
    }

    /// Default: "<name>: available" or "<name>: gone".
    fn status_line(&self) -> String {
        let state = if self.is_available() {
            "available"
        } else {
            "gone"
        };
        format!("{}: {state}", self.name())
    }
}

struct Feeder;
struct Disconnected;

impl Provider for Feeder {
    fn name(&self) -> String {
        "feeder".to_string()
    }
}

impl Provider for Disconnected {
    fn name(&self) -> String {
        "old-provider".to_string()
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply() {
        assert!(Feeder.is_available());
        assert_eq!(Feeder.status_line(), "feeder: available");
    }

    #[test]
    fn override_changes_default_behaviour_too() {
        assert!(!Disconnected.is_available());
        // status_line (a default) calls the *overridden* is_available.
        assert_eq!(Disconnected.status_line(), "old-provider: gone");
    }
}
