// c32_e04_test_builders.rs
//
// CONCEPT: table-driven tests with a small builder that describes each scenario.
// GUIDE:   chapters/32-testing/guide.md  (section "Table-driven tests and test builders")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs tests — `GetValuesConfigBuilder`
//             (`request_first().first_exist().auth_first()...build()`) + `test_get_values_combo`
// EXPECT:  tests fail: `todo!()` panics
//
// GetValues can fail in many combinations: signal missing, not authorized,
// no credentials at all, one of two signals bad... Writing one hand-made test
// per combination repeats the same setup over and over. The databroker's v2
// tests instead describe each scenario with a builder, and ONE helper turns a
// scenario into broker setup + request + expected result.
//
// This builder uses `&mut self -> &mut Self` methods (not the consuming
// `self -> Self` style from c29_e04): handy when you want to keep the builder
// around, e.g. to build variations. The trade-off: `build` must take `&self`
// and clone.
//
// Modifiers apply to the most recently added signal:
//     ScenarioBuilder::new().signal("A").exists().authorized().value(1.0)
//                           .signal("B").exists()
//                           .build()
//
// YOUR TASK
//   Implement the builder methods and `Scenario::run`, which must:
//     * put every `exists()` signal into the `Db` (with its value, if any),
//     * authorize exactly the `authorized()` signals (`Perms`),
//     * call `get_values` with ALL added signal paths, in order.
//   Do not edit `get_values` or the tests.

// I AM NOT DONE

use std::collections::{HashMap, HashSet};

// ---------------- code under test (do not change) ----------------

#[derive(Debug, PartialEq)]
enum GetError {
    NotFound(String),
    PermissionDenied(String),
}

type Db = HashMap<String, Option<f32>>;
type Perms = HashSet<String>;
type GetResult = Result<Vec<Option<f32>>, GetError>;

/// Fails on the FIRST problem, checking existence before permission (like the
/// databroker, it never reveals whether a signal you can't read exists... in
/// a real system you'd think hard about that order).
fn get_values(db: &Db, perms: &Perms, paths: &[&str]) -> GetResult {
    paths
        .iter()
        .map(|path| {
            let value = db
                .get(*path)
                .ok_or_else(|| GetError::NotFound(path.to_string()))?;
            if !perms.contains(*path) {
                return Err(GetError::PermissionDenied(path.to_string()));
            }
            Ok(*value)
        })
        .collect()
}

// ------------------------------ builder ------------------------------

#[derive(Debug, Clone, Default)]
struct SignalSpec {
    path: String,
    exists: bool,
    authorized: bool,
    value: Option<f32>,
}

#[derive(Debug, Clone)]
struct Scenario {
    signals: Vec<SignalSpec>,
}

#[derive(Default)]
struct ScenarioBuilder {
    signals: Vec<SignalSpec>,
}

impl ScenarioBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn signal(&mut self, path: &str) -> &mut Self {
        todo!()
    }

    /// Mutable access to the most recently added signal.
    fn last(&mut self) -> &mut SignalSpec {
        todo!("self.signals.last_mut().expect(...)")
    }

    fn exists(&mut self) -> &mut Self {
        todo!()
    }

    fn authorized(&mut self) -> &mut Self {
        todo!()
    }

    fn value(&mut self, value: f32) -> &mut Self {
        todo!()
    }

    fn build(&self) -> Scenario {
        todo!()
    }
}

impl Scenario {
    fn run(&self) -> GetResult {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn not_found(p: &str) -> GetError {
        GetError::NotFound(p.into())
    }
    fn denied(p: &str) -> GetError {
        GetError::PermissionDenied(p.into())
    }

    #[test]
    #[should_panic(expected = "call .signal(..) before modifying a signal")]
    fn modifier_without_signal_is_a_test_bug() {
        ScenarioBuilder::new().exists();
    }

    #[test]
    fn builder_can_be_reused_for_variations() {
        let mut base = ScenarioBuilder::new();
        base.signal("A").exists().authorized();
        let one = base.build();
        base.signal("B");
        let two = base.build();
        assert_eq!(one.signals.len(), 1);
        assert_eq!(two.signals.len(), 2);
    }

    #[test]
    fn table() {
        // (description, scenario, expected)
        let cases: Vec<(&str, Scenario, GetResult)> = vec![
            (
                "one readable signal with a value",
                ScenarioBuilder::new()
                    .signal("A")
                    .exists()
                    .authorized()
                    .value(1.0)
                    .build(),
                Ok(vec![Some(1.0)]),
            ),
            (
                "exists but no value yet",
                ScenarioBuilder::new()
                    .signal("A")
                    .exists()
                    .authorized()
                    .build(),
                Ok(vec![None]),
            ),
            (
                "missing signal",
                ScenarioBuilder::new().signal("A").authorized().build(),
                Err(not_found("A")),
            ),
            (
                "not authorized",
                ScenarioBuilder::new()
                    .signal("A")
                    .exists()
                    .value(1.0)
                    .build(),
                Err(denied("A")),
            ),
            (
                "second signal is the problem",
                ScenarioBuilder::new()
                    .signal("A")
                    .exists()
                    .authorized()
                    .value(1.0)
                    .signal("B")
                    .exists()
                    .build(),
                Err(denied("B")),
            ),
            (
                "order preserved",
                ScenarioBuilder::new()
                    .signal("B")
                    .exists()
                    .authorized()
                    .value(2.0)
                    .signal("A")
                    .exists()
                    .authorized()
                    .value(1.0)
                    .build(),
                Ok(vec![Some(2.0), Some(1.0)]),
            ),
        ];

        for (description, scenario, expected) in cases {
            assert_eq!(scenario.run(), expected, "case: {description}");
        }
    }
}
