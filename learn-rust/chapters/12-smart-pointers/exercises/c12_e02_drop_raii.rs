// c12_e02_drop_raii.rs
//
// CONCEPT: `Drop` and RAII — cleanup code that runs automatically when a
//          value goes out of scope; drop order; `drop(x)` to end early.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Drop and RAII guards")
// DATABROKER: databroker/src/broker.rs — lock guards from
//          `self.broker.database.read().await` are dropped at the end of the
//          statement/block, releasing the lock. `cleanup_change_subscriptions`
//          (line 892) notices a subscriber is gone because dropping its stream
//          dropped the broadcast `Receiver` (`receiver_count() == 0`).
// EXPECT:  tests fail: `todo!()` panics; once `subscribe` works, the
//          "unsubscribe" log lines are missing until you implement `Drop`.
//
// RAII = "Resource Acquisition Is Initialization": acquiring a resource
// creates a value; releasing it happens in that value's `Drop`. You can't
// forget to release, even on early `return` or `?`.
//
// Values are dropped at the end of their scope in REVERSE declaration order.
//
// `RefCell` (used for the log) is explained in the next exercise — just use
// `self.log.borrow_mut().push(...)`.
//
// YOUR TASK
//   1. Implement `Drop` for `SubscriptionHandle` so it pushes
//      "unsubscribe <name>" onto the log and decrements `active`.
//   2. Implement `Broker::subscribe` (increments `active`, pushes
//      "subscribe <name>", returns a handle).
//   3. Implement `short_lived_then_check` using `drop(...)`.
//   Do not edit the tests.

// I AM NOT DONE

use std::cell::{Cell, RefCell};

struct Broker {
    active: Cell<usize>,
    log: RefCell<Vec<String>>,
}

struct SubscriptionHandle<'b> {
    name: String,
    broker: &'b Broker,
}

impl Broker {
    fn new() -> Self {
        Broker {
            active: Cell::new(0),
            log: RefCell::new(Vec::new()),
        }
    }

    fn subscribe(&self, name: &str) -> SubscriptionHandle<'_> {
        todo!()
    }

    fn active(&self) -> usize {
        self.active.get()
    }
}

// TODO: impl Drop for SubscriptionHandle<'_> { ... }

/// Subscribes "temp", drops it explicitly, and returns how many
/// subscriptions are active right after the drop.
fn short_lived_then_check(broker: &Broker) -> usize {
    let handle = broker.subscribe("temp");
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_runs_at_end_of_scope_in_reverse_order() {
        let broker = Broker::new();
        {
            let _a = broker.subscribe("a");
            let _b = broker.subscribe("b");
            assert_eq!(broker.active(), 2);
        }
        assert_eq!(broker.active(), 0);
        assert_eq!(
            *broker.log.borrow(),
            vec![
                "subscribe a",
                "subscribe b",
                "unsubscribe b",
                "unsubscribe a"
            ]
        );
    }

    #[test]
    fn explicit_drop() {
        let broker = Broker::new();
        let _keep = broker.subscribe("keep");
        assert_eq!(short_lived_then_check(&broker), 1);
    }

    #[test]
    fn drop_runs_on_early_return() {
        fn fails(broker: &Broker) -> Result<(), String> {
            let _h = broker.subscribe("x");
            Err("boom".to_string())?;
            Ok(())
        }
        let broker = Broker::new();
        assert!(fails(&broker).is_err());
        assert_eq!(broker.active(), 0);
    }
}
