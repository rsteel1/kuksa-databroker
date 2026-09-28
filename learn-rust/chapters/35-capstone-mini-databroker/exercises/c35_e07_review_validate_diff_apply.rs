// c35_e07_review_validate_diff_apply.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs in the
//          three functions every update passes through: an off-by-one at a
//          min/max boundary, a change-detection comparison that looks at
//          the whole datapoint (timestamp included) instead of just its
//          value, and a tri-state match that only handles "set a new
//          actuator target", silently ignoring "clear it".
// GUIDE:   chapters/35-capstone-mini-databroker/guide.md  (section
//          "Step 2: updates and validation")
// DATABROKER: databroker/src/broker.rs:497-512 — `validate_value_min_max`
//             uses `greater_than_equal`/`less_than_equal`, so a value equal
//             to `min` or `max` is valid, not out of bounds;
//             databroker/src/broker.rs:276-289 — `Entry::diff` drops an
//             update whose `datapoint.value == self.datapoint.value` (only
//             `.value`, never the timestamp) for a non-`Continuous` signal;
//             databroker/src/broker.rs:758-772 — `Entry::apply` matches the
//             OUTER `Option` of `update.actuator_target` (`Some(target)`,
//             where `target: Option<Datapoint>`), so `Some(None)` — "clear
//             the target" — is applied exactly like `Some(Some(dp))`.
// EXPECT:  tests fail: a value exactly equal to `min` is rejected, sending
//          the same value twice on an on-change signal is reported as
//          "changed" both times, and clearing an actuator target (setting
//          it to `None`) is silently ignored.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests.

// I AM NOT DONE

use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
struct Datapoint {
    ts: SystemTime,
    value: f64,
}

impl Datapoint {
    fn now(value: f64) -> Self {
        Datapoint {
            ts: SystemTime::now(),
            value,
        }
    }
}

/// A partial update. `actuator_target` is tri-state: `None` = leave it
/// alone, `Some(None)` = clear it, `Some(Some(dp))` = set a new one.
#[derive(Debug, Clone, Default)]
struct EntryUpdate {
    datapoint: Option<Datapoint>,
    actuator_target: Option<Option<Datapoint>>,
}

#[derive(Debug, PartialEq)]
enum UpdateError {
    OutOfBoundsMinMax,
}

struct Entry {
    datapoint: Datapoint,
    actuator_target: Option<Datapoint>,
    min: Option<f64>,
    max: Option<f64>,
    /// `false` = on-change (dedupe identical values); `true` = always notify.
    continuous: bool,
}

impl Entry {
    fn validate_value(&self, value: f64) -> Result<(), UpdateError> {
        if let Some(min) = self.min {
            if value <= min {
                return Err(UpdateError::OutOfBoundsMinMax);
            }
        }
        if let Some(max) = self.max {
            if value > max {
                return Err(UpdateError::OutOfBoundsMinMax);
            }
        }
        Ok(())
    }

    /// Drops the datapoint from `update` if this is an on-change signal and
    /// its value hasn't actually changed.
    fn diff(&self, mut update: EntryUpdate) -> EntryUpdate {
        if !self.continuous {
            if let Some(dp) = &update.datapoint {
                if *dp == self.datapoint {
                    update.datapoint = None;
                }
            }
        }
        update
    }

    fn validate(&self, update: &EntryUpdate) -> Result<(), UpdateError> {
        if let Some(dp) = &update.datapoint {
            self.validate_value(dp.value)?;
        }
        if let Some(Some(target)) = &update.actuator_target {
            self.validate_value(target.value)?;
        }
        Ok(())
    }

    /// Applies the update and reports whether anything actually changed.
    fn apply(&mut self, update: EntryUpdate) -> bool {
        let mut changed = false;
        if let Some(dp) = update.datapoint {
            self.datapoint = dp;
            changed = true;
        }
        if let Some(Some(target)) = update.actuator_target {
            self.actuator_target = Some(target);
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            datapoint: Datapoint::now(50.0),
            actuator_target: None,
            min: Some(0.0),
            max: Some(100.0),
            continuous: false,
        }
    }

    #[test]
    fn a_value_equal_to_min_or_max_is_valid() {
        let e = entry();
        assert_eq!(e.validate_value(0.0), Ok(()));
        assert_eq!(e.validate_value(100.0), Ok(()));
        assert_eq!(
            e.validate_value(-0.001),
            Err(UpdateError::OutOfBoundsMinMax)
        );
        assert_eq!(
            e.validate_value(100.001),
            Err(UpdateError::OutOfBoundsMinMax)
        );
    }

    #[test]
    fn on_change_signal_drops_a_repeated_identical_value() {
        let e = entry();
        let update = EntryUpdate {
            datapoint: Some(Datapoint::now(50.0)), // same value, later timestamp
            ..Default::default()
        };
        let diffed = e.diff(update);
        assert!(
            diffed.datapoint.is_none(),
            "an unchanged value must be dropped, even with a newer timestamp"
        );
    }

    #[test]
    fn clearing_an_actuator_target_actually_clears_it() {
        let mut e = entry();
        e.actuator_target = Some(Datapoint::now(75.0));
        let changed = e.apply(EntryUpdate {
            actuator_target: Some(None),
            ..Default::default()
        });
        assert!(changed, "clearing the target is a change");
        assert_eq!(e.actuator_target, None);
    }

    #[test]
    fn setting_a_new_actuator_target_still_works() {
        let mut e = entry();
        let changed = e.apply(EntryUpdate {
            actuator_target: Some(Some(Datapoint::now(42.0))),
            ..Default::default()
        });
        assert!(changed);
        assert_eq!(e.actuator_target.map(|dp| dp.value), Some(42.0));
    }
}
