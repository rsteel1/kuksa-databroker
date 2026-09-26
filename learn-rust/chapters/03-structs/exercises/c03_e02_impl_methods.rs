// c03_e02_impl_methods.rs
//
// CONCEPT: `impl` blocks and the three kinds of `self`: `&self` (read),
//          `&mut self` (modify), `self` (consume).
// GUIDE:   chapters/03-structs/guide.md  (section "Methods and the three selves")
// DATABROKER: databroker/src/broker.rs — `Entry::validate(&self, ..)` reads,
//             `Entry::apply(&mut self, ..)` modifies, `Entry::diff(&self,
//             mut update: EntryUpdate) -> EntryUpdate` consumes the update.
// EXPECT:  tests fail: `todo!()` panics.
//
// The receiver tells callers exactly what a method will do to the value:
//   &self      "I only look"          (many callers may look at once)
//   &mut self  "I change you in place" (needs exclusive access)
//   self       "I take you over"      (the caller can't use it afterwards)
//
// YOUR TASK
//   Implement the four methods of `SignalStats`. Do not edit the tests.

// I AM NOT DONE

struct SignalStats {
    count: u32,
    sum: i64,
    min: i64,
    max: i64,
}

impl SignalStats {
    /// Starts empty. (Associated function — no `self`. Called as `SignalStats::empty()`.)
    fn empty() -> SignalStats {
        SignalStats {
            count: 0,
            sum: 0,
            min: i64::MAX,
            max: i64::MIN,
        }
    }

    /// Records one reading.
    fn record(&mut self, value: i64) {
        todo!()
    }

    /// Average of all readings, or 0.0 if there are none.
    fn average(&self) -> f64 {
        todo!()
    }

    /// Range spanned by the readings (max - min), or 0 if there are none.
    fn spread(&self) -> i64 {
        todo!()
    }

    /// Consumes the stats and produces a one-line report:
    /// "n=3 min=1 max=9 avg=4.33" (average with two decimals, `{:.2}`).
    /// With no readings: "n=0".
    fn into_report(self) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stats() {
        let stats = SignalStats::empty();
        assert_eq!(stats.average(), 0.0);
        assert_eq!(stats.spread(), 0);
        assert_eq!(stats.into_report(), "n=0");
    }

    #[test]
    fn records_readings() {
        let mut stats = SignalStats::empty();
        for v in [1, 9, 3] {
            stats.record(v);
        }
        assert_eq!(stats.count, 3);
        assert!((stats.average() - 13.0 / 3.0).abs() < 1e-9);
        assert_eq!(stats.spread(), 8);
        assert_eq!(stats.into_report(), "n=3 min=1 max=9 avg=4.33");
    }

    #[test]
    fn negative_readings() {
        let mut stats = SignalStats::empty();
        stats.record(-40);
        stats.record(-10);
        assert_eq!(stats.spread(), 30);
        assert_eq!(stats.average(), -25.0);
    }
}
