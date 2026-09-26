// c17_e07_panics_unwinding_poisoning.rs
//
// CONCEPT: what a panic actually does (unwinds the stack, running Drop),
//          stopping one at a boundary with `catch_unwind`, reading the panic
//          payload, mutex *poisoning* and how to recover from it, and
//          `#[track_caller]` for helpful panic locations.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Panics, unwinding and poisoning")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs:431 `None => todo!()`:
//             a panic reachable from client input. Tokio catches a panicking
//             task (like `catch_unwind` at the task boundary), so one bad
//             request doesn't kill the server, but the client gets a broken
//             connection instead of a clean `invalid_argument`. It would be
//             better as a `TryFrom` returning an error.
// EXPECT:  tests fail: `todo!()` panics
//
// Panic messages will appear in the test output (stderr) even when the tests
// pass. That's the default panic hook printing before `catch_unwind` catches
// the panic. It's expected.
//
// YOUR TASK
//   Implement `run_isolated`, `panic_message` and `read_recovering`, and make
//   `call_site_line` report the *caller's* line. Do not edit the tests.

use std::any::Any;
use std::panic::{self, UnwindSafe};
use std::sync::{Mutex, PoisonError};

/// Extracts a readable message from a panic payload. `panic!("literal")`
/// carries a `&'static str`; `panic!("{x}")` carries a `String`. Anything
/// else -> "unknown panic".
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Runs a plugin callback. If it panics, return Err("plugin <name> panicked: <message>")
/// instead of taking the whole host down.
fn run_isolated<F>(name: &str, plugin: F) -> Result<i32, String>
where
    F: FnOnce() -> i32 + UnwindSafe,
{
    panic::catch_unwind(plugin).map_err(|payload| {
        format!(
            "plugin {name} panicked: {}",
            panic_message(payload.as_ref())
        )
    })
}

/// Reads a counter even if another thread panicked while holding the lock.
/// A poisoned mutex still contains data. Decide whether it's still valid
/// (here: a plain counter, always valid) and use `PoisonError::into_inner`.
fn read_recovering(counter: &Mutex<u32>) -> u32 {
    // WHY: lock() returns Err(PoisonError<Guard>) when poisoned; into_inner gives the guard anyway.
    *counter.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the line number this function was *called from*. Without
/// `#[track_caller]`, `Location::caller()` reports this function's own line.
#[track_caller]
fn call_site_line() -> u32 {
    panic::Location::caller().line()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn payload_messages() {
        let p: Box<dyn Any + Send> = Box::new("static text");
        assert_eq!(panic_message(p.as_ref()), "static text");
        let p: Box<dyn Any + Send> = Box::new(format!("formatted {}", 42));
        assert_eq!(panic_message(p.as_ref()), "formatted 42");
        let p: Box<dyn Any + Send> = Box::new(7_u8);
        assert_eq!(panic_message(p.as_ref()), "unknown panic");
    }

    #[test]
    fn isolated_plugin_panics_become_errors() {
        assert_eq!(run_isolated("ok", || 5), Ok(5));
        assert_eq!(
            run_isolated("seat", || panic!("unsupported seat position")),
            Err("plugin seat panicked: unsupported seat position".to_string())
        );
        let id = 3;
        assert_eq!(
            run_isolated("hvac", move || panic!("zone {id} missing")),
            Err("plugin hvac panicked: zone 3 missing".to_string())
        );
    }

    #[test]
    fn drop_runs_during_unwinding() {
        struct Guard<'a>(&'a Mutex<Vec<&'static str>>);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.lock().unwrap().push("cleaned up");
            }
        }
        let log = Mutex::new(Vec::new());
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let _guard = Guard(&log);
            panic!("mid-operation");
        }));
        assert!(result.is_err());
        assert_eq!(*log.lock().unwrap(), vec!["cleaned up"]);
    }

    #[test]
    fn recovers_from_poisoned_mutex() {
        let counter = Arc::new(Mutex::new(41));
        let c = Arc::clone(&counter);
        let _ = thread::spawn(move || {
            let mut guard = c.lock().unwrap();
            *guard += 1;
            panic!("worker crashed while holding the lock");
        })
        .join();
        assert!(counter.is_poisoned());
        assert_eq!(read_recovering(&counter), 42);
    }

    #[test]
    fn track_caller_reports_the_call_site() {
        let expected = line!() + 1;
        let got = call_site_line();
        assert_eq!(got, expected);
    }
}
