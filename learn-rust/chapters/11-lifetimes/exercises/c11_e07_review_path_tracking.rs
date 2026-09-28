// c11_e07_review_path_tracking.rs
//
// CONCEPT: code review. Every function and method below has a signature
//          the borrow checker is perfectly happy with — the lifetimes are
//          all correct — but 3 of them are logically wrong: they compile,
//          borrow-check, and return a value of the right type, just not
//          the right value.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Choosing which input the output borrows from")
// DATABROKER: databroker/src/broker.rs — code that tracks "the current
//          entry" alongside a log/history must keep both in sync on every
//          update, exactly like `LatestPath` below is supposed to.
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. The borrow checker accepts all of them — that's
// what "lifetime-correct but logically stale" means. Passing `cargo check`
// is not the same as being right.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests, the struct, or the
//   signatures. For each bug, add a one-line comment directly above your
//   fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

/// Tracks the most recently seen path, plus a log of every path ever seen.
/// `current` must always equal the most recent argument passed to `update`
/// (or `initial`, before the first `update`).
struct LatestPath<'a> {
    current: &'a str,
    seen: Vec<&'a str>,
}

impl<'a> LatestPath<'a> {
    fn new(initial: &'a str) -> Self {
        LatestPath {
            current: initial,
            seen: vec![initial],
        }
    }

    fn update(&mut self, path: &'a str) {
        self.seen.push(path);
    }
}

/// The segment of `path` before the first `.`, or the whole string if
/// there's no `.` at all.
fn first_segment(path: &str) -> &str {
    path
}

/// Whichever of `a` or `b` is the longer string (`a` on a tie).
fn longer_of<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() < b.len() {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_path_current_tracks_the_last_update() {
        let mut lp = LatestPath::new("Vehicle.Speed");
        assert_eq!(lp.current, "Vehicle.Speed");
        lp.update("Vehicle.Cabin.Door");
        assert_eq!(lp.current, "Vehicle.Cabin.Door");
        lp.update("Vehicle.Powertrain");
        assert_eq!(lp.current, "Vehicle.Powertrain");
    }

    #[test]
    fn latest_path_seen_records_every_update_in_order() {
        let mut lp = LatestPath::new("Vehicle.Speed");
        lp.update("Vehicle.Cabin.Door");
        lp.update("Vehicle.Powertrain");
        assert_eq!(
            lp.seen,
            vec!["Vehicle.Speed", "Vehicle.Cabin.Door", "Vehicle.Powertrain"]
        );
    }

    #[test]
    fn first_segment_stops_at_the_first_dot() {
        assert_eq!(first_segment("Vehicle.Cabin.Door"), "Vehicle");
    }

    #[test]
    fn first_segment_of_a_dotless_path_is_the_whole_path() {
        assert_eq!(first_segment("Vehicle"), "Vehicle");
    }

    #[test]
    fn longer_of_picks_the_longer_string() {
        assert_eq!(
            longer_of("Vehicle.Speed", "Vehicle.Cabin.Door.Row1.Left"),
            "Vehicle.Cabin.Door.Row1.Left"
        );
        assert_eq!(longer_of("abc", "de"), "abc");
    }

    #[test]
    fn longer_of_breaks_ties_toward_a() {
        assert_eq!(longer_of("abc", "xyz"), "abc");
    }
}
