// c05_e01_vec.rs
//
// CONCEPT: Vec<T> — growable arrays: push, len, slices, remove, iterators.
// GUIDE:   chapters/05-collections/guide.md  (section "Vec: the workhorse")
// DATABROKER: databroker/src/broker.rs — `EntryUpdates { updates: Vec<ChangeNotification> }`
//             and the subscribe buffer_size rules in `AuthorizedAccess::subscribe`.
// EXPECT:  tests fail: `todo!()` panics
//
// A subscriber in the databroker can ask for a buffer of old values
// ("buffer_size"). Here you implement a tiny bounded history of speed
// readings using a Vec: newest values are pushed at the back and the oldest
// are dropped from the front once capacity is exceeded.
//
// YOUR TASK
//   Replace every `todo!()` in the `History` impl. Do not edit the tests.

// I AM NOT DONE

/// Bounded history of readings, oldest first.
#[derive(Debug)]
struct History {
    capacity: usize,
    readings: Vec<f32>,
}

impl History {
    fn new(capacity: usize) -> Self {
        todo!()
    }

    /// Adds a reading. If that makes the history longer than `capacity`,
    /// drop the oldest reading(s) so that exactly `capacity` remain.
    fn push(&mut self, value: f32) {
        todo!()
    }

    /// The most recent reading, if any.
    fn latest(&self) -> Option<f32> {
        todo!()
    }

    /// The `n` most recent readings, oldest first. If fewer than `n` exist,
    /// return all of them. Returns a *slice*: a view into the Vec, no copy.
    fn last_n(&self, n: usize) -> &[f32] {
        todo!()
    }

    /// Mean of all readings, or `None` if the history is empty.
    fn average(&self) -> Option<f32> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_history() {
        let h = History::new(3);
        assert_eq!(h.latest(), None);
        assert_eq!(h.average(), None);
        assert!(h.last_n(5).is_empty());
    }

    #[test]
    fn keeps_only_capacity_newest() {
        let mut h = History::new(3);
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            h.push(v);
        }
        assert_eq!(h.readings, vec![30.0, 40.0, 50.0]);
        assert_eq!(h.latest(), Some(50.0));
    }

    #[test]
    fn last_n_returns_tail_oldest_first() {
        let mut h = History::new(10);
        for v in [1.0, 2.0, 3.0, 4.0] {
            h.push(v);
        }
        assert_eq!(h.last_n(2), &[3.0, 4.0]);
        assert_eq!(h.last_n(100), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn average_of_readings() {
        let mut h = History::new(4);
        for v in [2.0, 4.0, 6.0, 8.0] {
            h.push(v);
        }
        assert_eq!(h.average(), Some(5.0));
    }

    #[test]
    fn capacity_zero_keeps_nothing() {
        let mut h = History::new(0);
        h.push(1.0);
        assert!(h.readings.is_empty());
    }
}
