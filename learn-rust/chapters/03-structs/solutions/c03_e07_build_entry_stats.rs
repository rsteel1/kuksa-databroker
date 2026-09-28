// c03_e07_build_entry_stats.rs
//
// CONCEPT: writing a struct and its methods from a spec: `&self` vs
//          `&mut self` vs `self`, associated functions, and `Default`.
// GUIDE:   chapters/03-structs/guide.md  (section "Associated functions: new and Default")
// DATABROKER: databroker/src/broker.rs — `Metadata`/`Datapoint`/`Entry`
//             track a signal's shape and its current value; this is a
//             small "running stats" companion for one signal.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. Design `EntryStats` and its methods so every test passes.
//
// YOUR TASK
//   Add the fields `EntryStats` needs, then implement `EntryStats::new`,
//   `record`, `range`, `average` and `into_summary`.
//   Do not edit the tests.

/// Running statistics for one signal's numeric readings.
struct EntryStats {
    path: String,
    count: u32,
    sum: f64,
    min: f64,
    max: f64,
}

impl EntryStats {
    /// A fresh tracker for the signal at `path`, with no readings yet.
    fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            count: 0,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }

    /// Folds one more reading into the running min/max/count/sum.
    fn record(&mut self, value: f64) {
        if self.count == 0 {
            self.min = value;
            self.max = value;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
        self.sum += value;
        self.count += 1;
    }

    /// `max - min` seen so far, or `0.0` if nothing was ever recorded.
    fn range(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.max - self.min
        }
    }

    /// The mean of every recorded value, or `0.0` if nothing was
    /// recorded.
    fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / f64::from(self.count)
        }
    }

    /// Consumes `self` and produces a one-line human-readable summary:
    /// `"<path>: n=<count> min=<min> max=<max> avg=<avg, 2 decimals>"`.
    fn into_summary(self) -> String {
        format!(
            "{}: n={} min={} max={} avg={:.2}",
            self.path,
            self.count,
            self.min,
            self.max,
            self.average()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_tracker_has_no_range_or_average() {
        let stats = EntryStats::new("Vehicle.Speed");
        assert_eq!(stats.range(), 0.0);
        assert_eq!(stats.average(), 0.0);
    }

    #[test]
    fn recording_updates_min_max_and_average() {
        let mut stats = EntryStats::new("Vehicle.Speed");
        stats.record(10.0);
        stats.record(30.0);
        stats.record(20.0);
        assert_eq!(stats.range(), 20.0);
        assert_eq!(stats.average(), 20.0);
    }

    #[test]
    fn summary_uses_the_path_and_two_decimals() {
        let mut stats = EntryStats::new("Vehicle.Speed");
        stats.record(10.0);
        stats.record(15.0);
        stats.record(16.0);
        assert_eq!(
            stats.into_summary(),
            "Vehicle.Speed: n=3 min=10 max=16 avg=13.67"
        );
    }
}
