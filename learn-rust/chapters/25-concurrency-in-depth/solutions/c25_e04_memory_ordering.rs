// c25_e04_memory_ordering.rs
//
// CONCEPT: memory ordering in practice — `Relaxed` for standalone counters,
//          `Release`/`Acquire` to publish data through a flag, `SeqCst` when
//          several flags must be seen in one global order.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Memory ordering in practice")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `next_request_id.fetch_add(1,
//             Ordering::Relaxed)` (line 174): a standalone id needs no ordering.
//             databroker/src/broker.rs — `next_id.fetch_add(1, Ordering::SeqCst)`
//             (line 1326) is stronger than necessary (it is also done under the
//             database write lock), which is harmless but worth noticing.
// EXPECT:  tests fail: `todo!()` panics
//
// CPUs and compilers reorder memory operations for speed. Each atomic
// operation says how much reordering around it is allowed:
//   Relaxed  - only this one variable is atomic; no ordering with other memory.
//   Release  - (on a store) all my earlier writes become visible to whoever
//              Acquire-loads this value.
//   Acquire  - (on a load) after I see the Release-stored value, I also see
//              everything written before that store.
//   SeqCst   - Acquire + Release + one single order of all SeqCst ops that
//              every thread agrees on.
// A wrong ordering usually still passes tests on x86 (strong hardware) and
// fails rarely on ARM — so you must reason, not rely on testing.
//
// YOUR TASK
//   1. `Mailbox::publish` / `Mailbox::try_read`: one writer publishes four
//      numbers once; readers must never see `ready == true` with stale data.
//      Pick the weakest correct ordering for each operation.
//   2. `choose_ordering`: answer, for each scenario, which ordering is the
//      weakest correct choice. The tests check your reasoning.
//   Do not edit the tests.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct Mailbox {
    data: [AtomicU64; 4],
    ready: AtomicBool,
}

impl Mailbox {
    fn new() -> Self {
        Mailbox {
            data: Default::default(),
            ready: AtomicBool::new(false),
        }
    }

    /// Writes the four values, then marks the mailbox ready.
    fn publish(&self, values: [u64; 4]) {
        for (slot, v) in self.data.iter().zip(values) {
            // WHY: Relaxed is fine for the payload — the Release store below
            // orders all of these writes before `ready` becomes true.
            slot.store(v, Ordering::Relaxed);
        }
        self.ready.store(true, Ordering::Release);
    }

    /// Returns the values once published, `None` before that.
    fn try_read(&self) -> Option<[u64; 4]> {
        // WHY: Acquire pairs with the Release in `publish`: seeing `true`
        // guarantees the payload writes are visible too.
        if !self.ready.load(Ordering::Acquire) {
            return None;
        }
        Some(std::array::from_fn(|i| {
            self.data[i].load(Ordering::Relaxed)
        }))
    }
}

#[derive(Debug, Clone, Copy)]
enum Scenario {
    /// Handing out unique request ids (`fetch_add`), nothing else depends on it.
    UniqueIdCounter,
    /// Storing `true` into a flag that tells readers "the config struct is ready".
    PublishFlagStore,
    /// Loading that flag before reading the config struct.
    PublishFlagLoad,
    /// Counting events for a metrics dashboard read occasionally.
    StatisticsCounter,
    /// Two threads each set their own flag, then check the OTHER's flag, and
    /// the algorithm requires that at least one of them sees the other's flag
    /// (Dekker/Peterson-style mutual exclusion).
    TwoFlagsMutualExclusion,
}

fn choose_ordering(scenario: Scenario) -> Ordering {
    match scenario {
        Scenario::UniqueIdCounter | Scenario::StatisticsCounter => Ordering::Relaxed,
        Scenario::PublishFlagStore => Ordering::Release,
        Scenario::PublishFlagLoad => Ordering::Acquire,
        // WHY: store-then-load on DIFFERENT variables can be reordered even
        // with Release/Acquire; only SeqCst's single global order forbids
        // both threads seeing the other's flag as false.
        Scenario::TwoFlagsMutualExclusion => Ordering::SeqCst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn nothing_before_publish() {
        let m = Mailbox::new();
        assert_eq!(m.try_read(), None);
        m.publish([1, 2, 3, 4]);
        assert_eq!(m.try_read(), Some([1, 2, 3, 4]));
    }

    #[test]
    fn readers_never_see_ready_with_stale_data() {
        for round in 0..200u64 {
            let m = Mailbox::new();
            let values = [round + 1, round + 2, round + 3, round + 4];
            thread::scope(|s| {
                for _ in 0..3 {
                    s.spawn(|| loop {
                        if let Some(seen) = m.try_read() {
                            assert_eq!(seen, values, "saw ready flag before the data");
                            break;
                        }
                        std::hint::spin_loop();
                    });
                }
                s.spawn(|| m.publish(values));
            });
        }
    }

    #[test]
    fn orderings_are_the_weakest_correct_ones() {
        use Scenario::*;
        assert_eq!(choose_ordering(UniqueIdCounter), Ordering::Relaxed);
        assert_eq!(choose_ordering(StatisticsCounter), Ordering::Relaxed);
        assert_eq!(choose_ordering(PublishFlagStore), Ordering::Release);
        assert_eq!(choose_ordering(PublishFlagLoad), Ordering::Acquire);
        assert_eq!(choose_ordering(TwoFlagsMutualExclusion), Ordering::SeqCst);
    }
}
