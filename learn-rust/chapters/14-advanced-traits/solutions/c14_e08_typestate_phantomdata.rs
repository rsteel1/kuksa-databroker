// c14_e08_typestate_phantomdata.rs
//
// CONCEPT: the typestate pattern — encode "which steps have happened" in a
//          type parameter, so calling `build()` too early is a *compile*
//          error — and `PhantomData`, which lets a struct carry a type
//          parameter it doesn't store.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Typestate and PhantomData")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::subscribe` checks at
//             RUNTIME that the request isn't empty (`SubscriptionError::InvalidInput`).
//             databroker/src/permissions.rs — `PermissionBuilder` is a
//             consuming builder whose `build()` returns a `Result`.
//             Here you'll build a subscription request where "no signals" is
//             impossible to express, so that error variant can't happen.
// EXPECT:  compile error E0392 (type parameter `S` is never used), then
//          `todo!()` panics.
//
// `SubscribeBuilder<NoSignals>` has no `build` method at all. Adding the
// first signal returns a `SubscribeBuilder<HasSignals>`, which does.
// `NoSignals`/`HasSignals` are empty structs used only as labels; they
// never exist at runtime, and `PhantomData<S>` costs zero bytes.
//
// YOUR TASK
//   1. Add a `PhantomData<S>` field so the struct uses `S`.
//   2. Implement the methods. Rules (same as the real broker):
//      - `buffer_size` values above 1000 are clamped to 1000;
//      - `interval_ms` is optional; default None; default buffer_size is 0;
//      - `signal` can be called on either state and always yields HasSignals.
//   Do not edit the tests.

use std::marker::PhantomData;

/// State labels (zero-sized; never constructed with data).
struct NoSignals;
struct HasSignals;

const MAX_SUBSCRIBE_BUFFER_SIZE: usize = 1000;

struct SubscribeBuilder<S> {
    signals: Vec<String>,
    buffer_size: usize,
    interval_ms: Option<u32>,
    state: PhantomData<S>,
}

#[derive(Debug, PartialEq)]
struct SubscribeRequest {
    signals: Vec<String>,
    buffer_size: usize,
    interval_ms: Option<u32>,
}

impl SubscribeBuilder<NoSignals> {
    fn new() -> Self {
        SubscribeBuilder {
            signals: Vec::new(),
            buffer_size: 0,
            interval_ms: None,
            state: PhantomData,
        }
    }
}

/// Methods available in EVERY state.
impl<S> SubscribeBuilder<S> {
    fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size.min(MAX_SUBSCRIBE_BUFFER_SIZE);
        self
    }

    fn interval_ms(mut self, ms: u32) -> Self {
        self.interval_ms = Some(ms);
        self
    }

    /// Adding a signal moves the builder into the `HasSignals` state.
    fn signal(self, path: &str) -> SubscribeBuilder<HasSignals> {
        // WHY: a different type parameter means a different type, so we
        // rebuild the struct, moving the fields across.
        let mut signals = self.signals;
        signals.push(path.to_string());
        SubscribeBuilder {
            signals,
            buffer_size: self.buffer_size,
            interval_ms: self.interval_ms,
            state: PhantomData,
        }
    }
}

/// `build` exists ONLY once at least one signal was added.
impl SubscribeBuilder<HasSignals> {
    fn build(self) -> SubscribeRequest {
        SubscribeRequest {
            signals: self.signals,
            buffer_size: self.buffer_size,
            interval_ms: self.interval_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_after_first_signal() {
        let request = SubscribeBuilder::new()
            .buffer_size(10)
            .signal("Vehicle.Speed")
            .signal("Vehicle.Rpm")
            .interval_ms(100)
            .build();
        assert_eq!(
            request,
            SubscribeRequest {
                signals: vec!["Vehicle.Speed".into(), "Vehicle.Rpm".into()],
                buffer_size: 10,
                interval_ms: Some(100),
            }
        );
    }

    #[test]
    fn defaults_and_clamping() {
        let request = SubscribeBuilder::new()
            .signal("A")
            .buffer_size(5000)
            .build();
        assert_eq!(request.buffer_size, MAX_SUBSCRIBE_BUFFER_SIZE);
        assert_eq!(request.interval_ms, None);
    }

    #[test]
    fn states_cost_nothing() {
        // The state label adds no bytes: both builders have the same size.
        assert_eq!(
            std::mem::size_of::<SubscribeBuilder<NoSignals>>(),
            std::mem::size_of::<SubscribeBuilder<HasSignals>>()
        );
        assert_eq!(std::mem::size_of::<PhantomData<HasSignals>>(), 0);
    }

    // Try it: uncomment the next line — it must NOT compile
    // (no method named `build` found for `SubscribeBuilder<NoSignals>`).
    // fn does_not_compile() { SubscribeBuilder::new().build(); }
}
