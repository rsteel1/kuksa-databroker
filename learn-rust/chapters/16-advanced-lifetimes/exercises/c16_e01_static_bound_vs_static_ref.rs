// c16_e01_static_bound_vs_static_ref.rs
//
// CONCEPT: `T: 'static` means "T holds no short-lived borrows", not "T lives forever"
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "'static: a bound, not a lifetime of the value")
// DATABROKER: databroker/src/broker.rs — `pub trait SignalProvider: Send + Sync + 'static`
//             and `Arc<dyn ActuationProvider + Send + Sync + 'static>`: providers are
//             moved into long-lived broker state, so they must not borrow anything.
// EXPECT:  compile error E0310 (the parameter type `T` may not live long enough)
//
// `std::thread::spawn` takes a closure that may run after the caller has
// returned, so everything the closure owns must be free of borrows that could
// expire: its bound is `F: Send + 'static`. A `String` satisfies `'static`
// (it owns its bytes), and so does `&'static str`. A `&str` borrowed from a
// local variable does not.
//
// Chapter 20 meets the same bound again on `tokio::spawn`.
//
// YOUR TASK
//   1. Fix `describe_in_background` by adding the bound the compiler asks for.
//   2. Implement `count_matching_scoped` with `std::thread::scope`, which *can*
//      borrow local data because the scope waits for its threads.
//   Do not edit the tests.

// I AM NOT DONE

use std::fmt::Debug;
use std::thread;

/// Formats `item` on a background thread and returns the handle.
// borrows that could expire: `T: 'static`. Owned data qualifies.
fn describe_in_background<T: Debug + Send>(item: T) -> thread::JoinHandle<String> {
    thread::spawn(move || format!("{item:?}"))
}

/// Counts the paths that start with `prefix`, splitting the work over two
/// threads that borrow `paths` directly (no cloning, no `Arc`).
fn count_matching_scoped(paths: &[String], prefix: &str) -> usize {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_values_are_static() {
        let path = String::from("Vehicle.Speed");
        let handle = describe_in_background(path); // moved in: fine
        assert_eq!(handle.join().unwrap(), "\"Vehicle.Speed\"");
    }

    #[test]
    fn static_references_are_static_too() {
        let literal: &'static str = "Vehicle.Width";
        assert_eq!(
            describe_in_background(literal).join().unwrap(),
            "\"Vehicle.Width\""
        );
        assert_eq!(describe_in_background(vec![1, 2]).join().unwrap(), "[1, 2]");
    }

    #[test]
    fn scoped_threads_may_borrow() {
        let paths: Vec<String> = [
            "Vehicle.Speed",
            "Vehicle.Cabin.Temp",
            "Other.X",
            "Vehicle.Width",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let prefix = String::from("Vehicle.");
        assert_eq!(count_matching_scoped(&paths, &prefix), 3);
        assert_eq!(count_matching_scoped(&[], "x"), 0);
        // `paths` is still ours afterwards: nothing was moved into the threads.
        assert_eq!(paths.len(), 4);
    }
}
