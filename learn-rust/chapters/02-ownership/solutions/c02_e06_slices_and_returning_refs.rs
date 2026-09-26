// c02_e06_slices_and_returning_refs.rs
//
// CONCEPT: returning references — you can only return a borrow of something
//          the *caller* owns, never of a local variable.
// GUIDE:   chapters/02-ownership/guide.md  (section "Returning references")
// DATABROKER: databroker/src/broker.rs — `DatabaseReadAccess::get_entry_by_id`
//             returns `Result<&Entry, ReadError>`: a borrow *into the
//             database*, which outlives the call.
// EXPECT:  compile error E0106 (missing lifetime specifier).
//
// When a function returns a reference, the compiler must know which input it
// borrows from. With one reference parameter it can guess ("elision"). With
// two, you must say it, using a *lifetime parameter* like `'a`:
//     fn pick<'a>(x: &'a str, y: &'a str) -> &'a str
// reads as "the result lives no longer than both x and y". Chapter 11 goes
// deep on lifetimes; here you only need this one idea.
//
// YOUR TASK
//   1. Fix `longer_path` by adding a lifetime parameter.
//   2. `default_branch` tries to return a reference to a local String that is
//      destroyed when the function returns (E0515 once E0106 is fixed).
//      Return an owned `String` instead.
//   3. Implement `window` — return the sub-slice `readings[start..start+len]`,
//      or an empty slice if that range does not fit.
//   Do not edit the tests.

/// Returns whichever path is longer (the first one on a tie).
fn longer_path<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.len() > a.len() {
        b
    } else {
        a
    }
}

fn default_branch() -> String {
    // WHY: return the String itself (moving ownership to the caller) —
    // a reference to a local would dangle once this function returns.
    String::from("Vehicle")
}

/// Borrow of a part of the caller's slice. One input reference, so the
/// compiler infers the lifetime: the output borrows from `readings`.
fn window(readings: &[u32], start: usize, len: usize) -> &[u32] {
    match start.checked_add(len) {
        Some(end) if end <= readings.len() => &readings[start..end],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer() {
        assert_eq!(
            longer_path("Vehicle.Speed", "Vehicle.Cabin.Lights"),
            "Vehicle.Cabin.Lights"
        );
        assert_eq!(longer_path("A.B", "C.D"), "A.B");
    }

    #[test]
    fn default() {
        assert_eq!(default_branch(), "Vehicle");
    }

    #[test]
    fn windows() {
        let readings = vec![1, 2, 3, 4, 5];
        assert_eq!(window(&readings, 1, 3), &[2, 3, 4]);
        assert_eq!(window(&readings, 4, 1), &[5]);
        assert_eq!(window(&readings, 4, 2), &[] as &[u32]);
        assert_eq!(window(&readings, usize::MAX, 2), &[] as &[u32]);
    }
}
