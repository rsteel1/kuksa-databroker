// c05_e06_retain_cleanup.rs
//
// CONCEPT: removing items while iterating — `retain` on Vec and HashMap.
// GUIDE:   chapters/05-collections/guide.md  (section "Removing while iterating: retain")
// DATABROKER: databroker/src/broker.rs — `Subscriptions::cleanup`,
//             `cleanup_change_subscriptions` and
//             `cleanup_signal_providers_subscriptions`, called every second
//             by the housekeeping task.
// EXPECT:  compile error E0502 (cannot borrow `self.subscriptions` as mutable
//          because it is also borrowed as immutable), then failing tests.
//
// You cannot call `map.remove(k)` inside `for (k, v) in &map` — the loop
// holds a shared borrow of the map while `remove` needs an exclusive one.
// The borrow checker stops you (in other languages this is the classic
// "ConcurrentModificationException" or silent iterator corruption).
// `retain(|k, v| keep?)` does both jobs in one pass.
//
// YOUR TASK
//   Rewrite `cleanup` using `HashMap::retain`, collecting the removed ids
//   (sorted). Then implement `drop_unavailable` with `Vec::retain`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;

struct ChangeSubscription {
    /// How many stream receivers are still alive (0 = client went away).
    receiver_count: usize,
    /// Token expiry as a unix timestamp in seconds, if any.
    expires_at: Option<u64>,
}

struct ActuationProvider {
    name: String,
    available: bool,
}

#[derive(Default)]
struct Subscriptions {
    subscriptions: HashMap<u32, ChangeSubscription>,
    actuation_providers: Vec<ActuationProvider>,
}

impl Subscriptions {
    /// Removes subscriptions nobody listens to anymore, or whose token has
    /// expired (`expires_at < now`). Returns the removed ids, sorted.
    fn cleanup(&mut self, now: u64) -> Vec<u32> {
        let mut removed = Vec::new();
        for (id, sub) in &self.subscriptions {
            let expired = sub.expires_at.is_some_and(|t| t < now);
            if sub.receiver_count == 0 || expired {
                self.subscriptions.remove(id);
                removed.push(*id);
            }
        }
        removed
    }

    /// Drops providers that are no longer available; returns their names
    /// in their original order.
    fn drop_unavailable(&mut self) -> Vec<String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(receiver_count: usize, expires_at: Option<u64>) -> ChangeSubscription {
        ChangeSubscription {
            receiver_count,
            expires_at,
        }
    }

    #[test]
    fn removes_dead_and_expired() {
        let mut s = Subscriptions::default();
        s.subscriptions.insert(1, sub(1, None)); // alive
        s.subscriptions.insert(2, sub(0, None)); // client gone
        s.subscriptions.insert(3, sub(2, Some(100))); // expired at now=150
        s.subscriptions.insert(4, sub(1, Some(200))); // still valid
        s.subscriptions.insert(5, sub(0, Some(10))); // both

        assert_eq!(s.cleanup(150), vec![2, 3, 5]);
        let mut left: Vec<u32> = s.subscriptions.keys().copied().collect();
        left.sort_unstable();
        assert_eq!(left, vec![1, 4]);
        assert!(s.cleanup(150).is_empty(), "second run removes nothing");
    }

    #[test]
    fn drops_unavailable_providers_in_order() {
        let mut s = Subscriptions::default();
        for (name, available) in [("a", true), ("b", false), ("c", false), ("d", true)] {
            s.actuation_providers.push(ActuationProvider {
                name: name.to_string(),
                available,
            });
        }
        assert_eq!(s.drop_unavailable(), vec!["b".to_string(), "c".to_string()]);
        let names: Vec<&str> = s
            .actuation_providers
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "d"]);
    }
}
