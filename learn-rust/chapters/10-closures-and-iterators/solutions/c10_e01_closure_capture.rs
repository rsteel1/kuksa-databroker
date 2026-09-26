// c10_e01_closure_capture.rs
//
// CONCEPT: closures capture their environment — by reference, by mutable
//          reference, or by value (`move`).
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Closures and what they capture")
// DATABROKER: databroker/src/broker.rs — `start_housekeeping_task` uses
//          `tokio::spawn(async move { ... })` so the task *owns* its clones.
// EXPECT:  compile errors E0373 (closure may outlive the current function)
//          and E0596 (cannot borrow `record` as mutable).
//
// A closure is an anonymous function that can use variables from the scope
// it was created in. How it uses them decides how it captures them:
//   - only reads      -> captures `&T`
//   - mutates         -> captures `&mut T`  (the closure itself must be `mut`)
//   - `move` keyword  -> takes ownership of what it uses
//
// YOUR TASK
//   Fix `above_limit` and `record_all` so the file compiles and the tests
//   pass. Do not edit the tests.

/// Returns a predicate that is true for speeds strictly above `limit`.
fn above_limit(limit: u32) -> impl Fn(u32) -> bool {
    // WHY: `move` copies `limit` into the closure, so the closure no longer
    // borrows a local that dies when this function returns.
    move |speed| speed > limit
}

/// Counts readings that satisfy `pred`.
fn count_matching(readings: &[u32], pred: impl Fn(u32) -> bool) -> usize {
    readings.iter().filter(|r| pred(**r)).count()
}

/// Records every reading into a Vec using a closure that mutates `seen`.
fn record_all(readings: &[u32]) -> Vec<u32> {
    let mut seen = Vec::new();
    // WHY: calling a closure that mutates its captures needs `&mut closure`.
    let mut record = |r: u32| seen.push(r);
    for r in readings {
        record(*r);
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_outlives_its_limit_variable() {
        let pred = {
            let limit = 100;
            above_limit(limit)
        };
        assert!(pred(101));
        assert!(!pred(100));
        assert_eq!(count_matching(&[10, 120, 130, 90], pred), 2);
    }

    #[test]
    fn record_keeps_order() {
        assert_eq!(record_all(&[3, 1, 2]), vec![3, 1, 2]);
    }
}
