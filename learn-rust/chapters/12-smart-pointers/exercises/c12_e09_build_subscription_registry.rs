// c12_e09_build_subscription_registry.rs
//
// CONCEPT: an `Rc<RefCell<T>>` shared handle plus `Weak` on the registry's
//          side, so the registry never keeps a dropped subscriber alive.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Weak: breaking reference cycles")
// DATABROKER: databroker/src/broker.rs — `Subscriptions::cleanup` drops
//          subscriptions whose receiver is gone; this is the same idea with
//          `Weak` instead of `receiver_count() == 0`, and single-threaded.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the three `todo!()` bodies below.
//
// A `SharedLog` is a subscriber's private log of values it has received.
// The registry only ever holds a `Weak` to it — when the subscriber drops
// its handle, the registry must notice and stop delivering to it, without
// the subscriber having to unsubscribe explicitly.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests or the type signatures.
//
//   1. `subscribe`: create a fresh `SharedLog`, store a `Weak` to it in
//      `self.subs` under `path` (create the `Vec` if `path` is new), and
//      return the strong handle to the caller.
//   2. `publish`: for every subscriber registered under `path`, if its weak
//      handle still upgrades, push `value` onto its log; if it doesn't
//      (the subscriber was dropped), drop that entry from `self.subs`
//      instead. Return how many *live* subscribers received `value`.
//   3. `live_count`: how many subscribers under `path` still upgrade.

// I AM NOT DONE

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

type SharedLog = Rc<RefCell<Vec<i32>>>;

#[derive(Default)]
struct SubscriptionRegistry {
    subs: HashMap<String, Vec<Weak<RefCell<Vec<i32>>>>>,
}

impl SubscriptionRegistry {
    fn new() -> Self {
        Self::default()
    }

    /// Registers a new subscriber under `path` and returns its handle.
    fn subscribe(&mut self, path: &str) -> SharedLog {
        todo!()
    }

    /// Delivers `value` to every live subscriber under `path`, pruning dead
    /// ones. Returns the number of live subscribers that received it.
    fn publish(&mut self, path: &str, value: i32) -> usize {
        todo!()
    }

    /// How many subscribers under `path` are still alive.
    fn live_count(&self, path: &str) -> usize {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_receive_published_values_in_order() {
        let mut registry = SubscriptionRegistry::new();
        let log = registry.subscribe("Vehicle.Speed");
        registry.publish("Vehicle.Speed", 1);
        registry.publish("Vehicle.Speed", 2);
        assert_eq!(*log.borrow(), vec![1, 2]);
    }

    #[test]
    fn different_paths_do_not_cross_deliver() {
        let mut registry = SubscriptionRegistry::new();
        let speed_log = registry.subscribe("Vehicle.Speed");
        let cabin_log = registry.subscribe("Vehicle.Cabin");
        registry.publish("Vehicle.Speed", 42);
        assert_eq!(*speed_log.borrow(), vec![42]);
        assert_eq!(*cabin_log.borrow(), Vec::<i32>::new());
    }

    #[test]
    fn publish_returns_the_live_subscriber_count() {
        let mut registry = SubscriptionRegistry::new();
        let _a = registry.subscribe("Vehicle.Speed");
        let _b = registry.subscribe("Vehicle.Speed");
        assert_eq!(registry.publish("Vehicle.Speed", 1), 2);
        assert_eq!(registry.publish("Vehicle.Unknown", 1), 0);
    }

    #[test]
    fn dropped_subscribers_stop_receiving_and_are_pruned() {
        let mut registry = SubscriptionRegistry::new();
        let a = registry.subscribe("Vehicle.Speed");
        let b = registry.subscribe("Vehicle.Speed");
        assert_eq!(registry.live_count("Vehicle.Speed"), 2);

        drop(a);
        let delivered = registry.publish("Vehicle.Speed", 7);
        assert_eq!(delivered, 1);
        assert_eq!(*b.borrow(), vec![7]);
        assert_eq!(registry.live_count("Vehicle.Speed"), 1);
    }
}
