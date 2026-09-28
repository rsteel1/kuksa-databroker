// c16_e07_build_window_visitor.rs
//
// CONCEPT: a higher-ranked closure bound (`for<'s> Fn(&'s [T])`) for a
//          visitor that's handed a freshly-borrowed slice on every call,
//          and a `'static` closure that proves `'static` means "owns its
//          data", not "the data lives forever" — it survives its creator
//          being dropped, and can be handed to `thread::spawn`.
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "Higher-ranked trait bounds (for<'a>)")
// DATABROKER: databroker/src/broker.rs's history/window-style buffers walk
//          runs of consecutive samples the way `for_each_window` does here;
//          `SignalProvider: Send + Sync + 'static` (chapter 13) is the same
//          "owns its data" rule `PathLogger::make_logger` demonstrates.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the three `todo!()` bodies below.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests, the struct, or the signatures.
//
//   1. `for_each_window`: call `f` once per contiguous window of `size`
//      elements of `data`, oldest first (`data.windows(size)` already
//      does the chunking — you just need to drive it).
//   2. `PathLogger::new`: store `label` (converted to an owned `String`)
//      and an empty `entries`.
//   3. `PathLogger::make_logger`: return a closure that formats
//      `"{label}: {path}"`. The closure must be usable after `self` is
//      dropped and sendable to another thread, so it can only capture an
//      owned copy of the label — not a reference to `self`.

/// Calls `f` once for every window of `size` consecutive elements of
/// `data`, oldest first. `f` must accept a slice of ANY lifetime (that's
/// what `for<'s>` says) because a new, shorter-lived slice is produced for
/// every window.
fn for_each_window<F>(data: &[i32], size: usize, f: F)
where
    F: for<'s> Fn(&'s [i32]),
{
    for window in data.windows(size) {
        f(window);
    }
}

struct PathLogger {
    label: String,
    #[allow(dead_code)]
    entries: Vec<String>,
}

impl PathLogger {
    fn new(label: impl Into<String>) -> Self {
        PathLogger {
            label: label.into(),
            entries: Vec::new(),
        }
    }

    /// A `'static` closure that formats `"{label}: {path}"`, independent
    /// of `self`'s lifetime.
    fn make_logger(&self) -> impl Fn(&str) -> String + 'static {
        // WHY: cloning `label` into an owned `String` (rather than
        // capturing `&self.label`) is what lets this closure outlive
        // `self` and cross a thread boundary — `'static` means "no
        // borrows inside", not "kept alive forever".
        let label = self.label.clone();
        move |path: &str| format!("{label}: {path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::thread;

    #[test]
    fn for_each_window_visits_every_contiguous_run() {
        let data = [1, 2, 3, 4, 5];
        let collected = RefCell::new(Vec::new());
        for_each_window(&data, 3, |w| collected.borrow_mut().push(w.to_vec()));
        assert_eq!(
            *collected.borrow(),
            vec![vec![1, 2, 3], vec![2, 3, 4], vec![3, 4, 5]]
        );
    }

    #[test]
    fn for_each_window_of_size_one_visits_every_element() {
        let data = [10, 20, 30];
        let collected = RefCell::new(Vec::new());
        for_each_window(&data, 1, |w| collected.borrow_mut().push(w[0]));
        assert_eq!(*collected.borrow(), vec![10, 20, 30]);
    }

    #[test]
    fn make_logger_outlives_its_creator() {
        let logger = PathLogger::new("provider-1");
        let log = logger.make_logger();
        drop(logger);
        assert_eq!(log("Vehicle.Speed"), "provider-1: Vehicle.Speed");
    }

    #[test]
    fn make_logger_can_cross_a_thread_boundary() {
        let logger = PathLogger::new("bg");
        let log = logger.make_logger();
        let handle = thread::spawn(move || log("Vehicle.Cabin"));
        assert_eq!(handle.join().unwrap(), "bg: Vehicle.Cabin");
    }
}
