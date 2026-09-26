// c13_e05_checkpoint_part1.rs
//
// CONCEPT: CHECKPOINT for Part I — ownership, structs, enums, collections,
//          errors, traits, iterators, lifetimes, smart pointers and threads,
//          all in one problem.
// GUIDE:   chapters/13-threads/guide.md  (section "Checkpoint")
// DATABROKER: databroker/src/broker.rs — `Database` (path → value storage),
//             `DatabaseReadAccess` (a borrowing view), `update_entries`
//             (collects every error instead of stopping at the first)
// EXPECT:  tests fail: `todo!()` panics (and `under` yields nothing)
//
// A provider has written a log of signal updates, one per line:
//
//     # comment lines and blank lines are ignored
//     Vehicle.Speed = 42
//     Vehicle.Cabin.Door.Row1.IsOpen=1
//
// Parse the log, apply the updates with several worker threads sharing one
// store, and expose the result as a read-only snapshot with a borrowing
// query API. There is no step-by-step guide for this one: the signatures and
// the tests are the specification.
//
// YOUR TASK
//   Implement every `todo!()`. You may add private helper functions, fields
//   and derives, but keep the public signatures and do not edit the tests.
//   Rules the tests pin down:
//   * `parse_line` trims the line. Blank lines and lines starting with `#`
//     give `Ok(None)`. Otherwise the line must be `path = value` (spaces
//     around `=` optional), with a non-empty path and an `i64` value.
//   * `ingest` reports EVERY bad line (in line order, line numbers start at
//     1), not just the first. With no errors it applies all updates.
//   * Updates are split across `workers` threads (treat 0 as 1). The result
//     must not depend on thread timing: when a path appears several times,
//     the update from the LATEST LINE wins.
//   * `Snapshot::under("A.B")` yields `A.B` itself and everything below it
//     (`A.B.C`), but not `A.BC`. It yields results sorted by path.

// I AM NOT DONE

#![allow(unused_imports, unused_variables)]
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::Bound;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub line: usize,
    pub path: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineError {
    MissingEquals { line: usize },
    EmptyPath { line: usize },
    BadValue { line: usize, text: String },
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Messages (see the tests): "line 7: expected `path = value`",
        // "line 2: empty signal path", "line 4: `fast` is not an integer"
        todo!()
    }
}

impl std::error::Error for LineError {}

pub fn parse_line(line: &str, line_no: usize) -> Result<Option<Update>, LineError> {
    todo!()
}

/// Read-only result of an ingest. Choose the fields yourself.
#[derive(Debug, Default)]
pub struct Snapshot {
    // your fields here
}

impl Snapshot {
    pub fn get(&self, path: &str) -> Option<i64> {
        todo!()
    }

    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// Every `(path, value)` at or below the branch `prefix`, sorted by path.
    /// The returned iterator borrows both the snapshot and `prefix`.
    pub fn under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, i64)> + 'a {
        // Replace this placeholder (it compiles, but yields nothing).
        let _ = prefix;
        std::iter::empty()
    }
}

pub fn ingest(log: &str, workers: usize) -> Result<Snapshot, Vec<LineError>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
# initial values
Vehicle.Speed = 10
Vehicle.Cabin.Door.Row1.IsOpen=1

Vehicle.Cabin.Temperature = 21
Vehicle.CabinLight = 3
Vehicle.Speed = 20
Vehicle.Cabin.Temperature=19
Vehicle.Speed = 30
";

    #[test]
    fn parse_line_accepts_updates_and_skips_noise() {
        assert_eq!(
            parse_line("  Vehicle.Speed = -5 ", 3),
            Ok(Some(Update {
                line: 3,
                path: "Vehicle.Speed".into(),
                value: -5
            }))
        );
        assert_eq!(parse_line("   ", 1), Ok(None));
        assert_eq!(parse_line("# Vehicle.Speed = 1", 1), Ok(None));
    }

    #[test]
    fn parse_line_reports_what_went_wrong() {
        assert_eq!(
            parse_line("Vehicle.Speed 42", 7),
            Err(LineError::MissingEquals { line: 7 })
        );
        assert_eq!(
            parse_line(" = 42", 2),
            Err(LineError::EmptyPath { line: 2 })
        );
        assert_eq!(
            parse_line("Vehicle.Speed = fast", 4),
            Err(LineError::BadValue {
                line: 4,
                text: "fast".into()
            })
        );
        assert_eq!(
            LineError::BadValue {
                line: 4,
                text: "fast".into()
            }
            .to_string(),
            "line 4: `fast` is not an integer"
        );
    }

    #[test]
    fn ingest_collects_every_error_in_order() {
        let log = "A.B = 1\nbroken\nA.C = x\n# fine\n = 3\n";
        let errors = ingest(log, 2).expect_err("log has three bad lines");
        assert_eq!(
            errors,
            vec![
                LineError::MissingEquals { line: 2 },
                LineError::BadValue {
                    line: 3,
                    text: "x".into()
                },
                LineError::EmptyPath { line: 5 },
            ]
        );
    }

    #[test]
    fn latest_line_wins_for_any_number_of_workers() {
        for workers in [0, 1, 2, 3, 8, 100] {
            let snapshot = ingest(LOG, workers).expect("log is valid");
            assert_eq!(snapshot.get("Vehicle.Speed"), Some(30), "workers={workers}");
            assert_eq!(
                snapshot.get("Vehicle.Cabin.Temperature"),
                Some(19),
                "workers={workers}"
            );
            assert_eq!(snapshot.len(), 4, "workers={workers}");
        }
    }

    #[test]
    fn empty_log_gives_empty_snapshot() {
        let snapshot = ingest("# nothing\n\n", 4).expect("comments only");
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.get("Vehicle.Speed"), None);
    }

    #[test]
    fn under_matches_whole_branches_sorted() {
        let snapshot = ingest(LOG, 3).expect("log is valid");
        let cabin: Vec<(&str, i64)> = snapshot.under("Vehicle.Cabin").collect();
        assert_eq!(
            cabin,
            vec![
                ("Vehicle.Cabin.Door.Row1.IsOpen", 1),
                ("Vehicle.Cabin.Temperature", 19),
            ]
        );
        let exact: Vec<_> = snapshot.under("Vehicle.Speed").collect();
        assert_eq!(exact, vec![("Vehicle.Speed", 30)]);
        let all: Vec<_> = snapshot.under("Vehicle").map(|(path, _)| path).collect();
        assert_eq!(
            all,
            vec![
                "Vehicle.Cabin.Door.Row1.IsOpen",
                "Vehicle.Cabin.Temperature",
                "Vehicle.CabinLight",
                "Vehicle.Speed",
            ]
        );
        assert_eq!(snapshot.under("Truck").count(), 0);
    }

    #[test]
    fn snapshot_can_be_shared_across_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Snapshot>();
        assert_send_sync::<LineError>();
    }
}
