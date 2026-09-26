// c12_e03_rc_refcell.rs
//
// CONCEPT: `Rc<T>` (shared ownership, single thread) and `RefCell<T>`
//          (borrow rules checked at runtime → interior mutability).
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Rc and RefCell: shared, single-threaded")
// DATABROKER: the databroker is multi-threaded, so it uses the thread-safe
//          cousins `Arc` + `RwLock` (next exercises). `Rc/RefCell` teach the
//          same ideas without threads.
// EXPECT:  tests fail: `todo!()` panics.
//
// `Rc<T>`: several owners of one value; the value is dropped when the last
//          `Rc` is dropped. `Rc::clone` only bumps a counter.
// `RefCell<T>`: lets you mutate through a shared reference. The usual rule
//          (many readers XOR one writer) is checked at RUNTIME: violating it
//          panics with `already borrowed`. `try_borrow` returns an error
//          instead of panicking.
//
// YOUR TASK
//   Implement `Registry::add`, `Registry::notify`, and `peek_len`.
//   Do not edit the tests.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Subscriber {
    received: Vec<i32>,
}

type SharedSubscriber = Rc<RefCell<Subscriber>>;

#[derive(Default)]
struct Registry {
    subscribers: Vec<SharedSubscriber>,
}

impl Registry {
    /// Stores a clone of the handle (shared ownership — the caller keeps theirs).
    fn add(&mut self, subscriber: &SharedSubscriber) {
        // WHY: `Rc::clone(x)` (rather than `x.clone()`) makes it obvious that
        // only a counter is bumped, not the Subscriber deep-copied.
        self.subscribers.push(Rc::clone(subscriber));
    }

    /// Pushes `value` into every subscriber's `received`.
    fn notify(&self, value: i32) {
        for subscriber in &self.subscribers {
            // WHY: `&self` is enough — RefCell grants mutable access at runtime.
            subscriber.borrow_mut().received.push(value);
        }
    }
}

/// Number of received values, or None if the subscriber is currently
/// mutably borrowed somewhere (use `try_borrow`, never panic).
fn peek_len(subscriber: &SharedSubscriber) -> Option<usize> {
    subscriber.try_borrow().ok().map(|s| s.received.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_subscribers_see_notifications() {
        let a: SharedSubscriber = Rc::default();
        let b: SharedSubscriber = Rc::default();
        let mut registry = Registry::default();
        registry.add(&a);
        registry.add(&b);
        assert_eq!(Rc::strong_count(&a), 2); // ours + the registry's

        registry.notify(1);
        registry.notify(2);
        assert_eq!(a.borrow().received, vec![1, 2]);
        assert_eq!(b.borrow().received, vec![1, 2]);

        drop(registry);
        assert_eq!(Rc::strong_count(&a), 1);
    }

    #[test]
    fn peek_does_not_panic_while_mutably_borrowed() {
        let a: SharedSubscriber = Rc::default();
        a.borrow_mut().received.push(7);
        assert_eq!(peek_len(&a), Some(1));

        let guard = a.borrow_mut(); // someone is writing…
        assert_eq!(peek_len(&a), None);
        drop(guard);
        assert_eq!(peek_len(&a), Some(1));
    }
}
