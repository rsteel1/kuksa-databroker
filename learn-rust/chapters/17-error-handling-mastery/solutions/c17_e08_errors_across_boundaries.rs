// c17_e08_errors_across_boundaries.rs
//
// CONCEPT: errors that cross boundaries: threads (the error must be `Send`),
//          type erasure (`Box<dyn Error + Send + Sync + 'static>`, and why each
//          bound is there), getting the concrete type back with `downcast_ref`,
//          and the two iterator strategies: fail fast (`collect` into
//          `Result<Vec<_>, _>`) vs collect every error (`partition`).
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Errors across boundaries: threads, trait objects, iterators")
// DATABROKER: databroker/src/broker.rs:1529 `update_entries` gathers *all*
//             per-signal failures into `Err(Vec<(i32, UpdateError)>)` so one bad
//             signal doesn't hide the others; grpc/server.rs:203 uses
//             `Into<Box<dyn std::error::Error + Send + Sync>>` as the bound for
//             connection errors.
// EXPECT:  compile error E0277: `(dyn std::error::Error + 'static)` cannot be
//          sent between threads safely
//
// YOUR TASK
//   1. Fix the `BoxError` alias so errors can travel between threads.
//   2. Implement the `todo!()` functions. Do not edit the tests.

use std::error::Error;
use std::num::ParseIntError;
use std::thread;

// WHY: Send lets the Box move to another thread, Sync lets &BoxError be shared,
//      and 'static (the default for Box<dyn ..>) means it borrows nothing.
type BoxError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
#[error("worker for {path} crashed")]
struct WorkerPanicked {
    path: String,
}

/// Parses `raw` on a worker thread. The worker's `Result<u32, BoxError>` is
/// handed back through `join()`. If the worker *panicked* instead, return a
/// `WorkerPanicked` error.
fn parse_on_worker(path: &str, raw: String) -> Result<u32, BoxError> {
    let handle = thread::spawn(move || -> Result<u32, BoxError> {
        if raw == "PANIC" {
            panic!("worker exploded");
        }
        Ok(raw.trim().parse::<u32>()?)
    });
    handle.join().unwrap_or_else(|_panic| {
        Err(Box::new(WorkerPanicked {
            path: path.to_string(),
        }))
    })
}

/// If the boxed error is really a ParseIntError, return it.
fn as_parse_error(err: &BoxError) -> Option<&ParseIntError> {
    err.downcast_ref::<ParseIntError>()
}

/// All-or-nothing: the first bad input stops the parse.
fn parse_all_fail_fast(inputs: &[&str]) -> Result<Vec<u32>, ParseIntError> {
    inputs.iter().map(|s| s.parse::<u32>()).collect()
}

/// Best effort: parse everything, returning the good values and, for each
/// bad input, its index and error (like `update_entries`).
fn parse_all_report_errors(inputs: &[&str]) -> (Vec<u32>, Vec<(usize, ParseIntError)>) {
    let (ok, bad): (Vec<_>, Vec<_>) = inputs
        .iter()
        .enumerate()
        .map(|(i, s)| s.parse::<u32>().map_err(|e| (i, e)))
        .partition(Result::is_ok);
    (
        ok.into_iter().flatten().collect(),
        bad.into_iter().filter_map(Result::err).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn box_error_can_cross_threads() {
        assert_send_sync::<BoxError>();
    }

    #[test]
    fn worker_ok_and_parse_error() {
        assert_eq!(parse_on_worker("Vehicle.Speed", " 42 ".into()).unwrap(), 42);
        let err = parse_on_worker("Vehicle.Speed", "fast".into()).unwrap_err();
        assert!(as_parse_error(&err).is_some());
    }

    #[test]
    fn worker_panic_becomes_an_error() {
        let err = parse_on_worker("Vehicle.Speed", "PANIC".into()).unwrap_err();
        assert!(as_parse_error(&err).is_none());
        assert_eq!(err.to_string(), "worker for Vehicle.Speed crashed");
        assert!(err.downcast_ref::<WorkerPanicked>().is_some());
    }

    #[test]
    fn fail_fast() {
        assert_eq!(parse_all_fail_fast(&["1", "2"]), Ok(vec![1, 2]));
        assert!(parse_all_fail_fast(&["1", "x", "3"]).is_err());
    }

    #[test]
    fn report_all_errors() {
        let (ok, bad) = parse_all_report_errors(&["1", "x", "3", "-4"]);
        assert_eq!(ok, vec![1, 3]);
        let indexes: Vec<usize> = bad.iter().map(|(i, _)| *i).collect();
        assert_eq!(indexes, vec![1, 3]);
    }
}
