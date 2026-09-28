// c11_e06_build_vss_path_parser.rs
//
// CONCEPT: writing your own lifetime-carrying parser: choosing which input
//          an output borrows from, and returning slices instead of owned
//          `String`s.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Choosing which input the output borrows from")
// DATABROKER: databroker/src/glob.rs:202 `is_valid_path` and the dotted VSS
//          path handling throughout `broker.rs`/`glob.rs` — paths are always
//          borrowed `&str`, never re-allocated just to inspect them.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the four `todo!()` bodies below.
//
// VSS signal paths are dot-separated, e.g. "Vehicle.Cabin.Door.Row1.Left".
// None of the functions below need to allocate: every returned `&str` is a
// slice of one of the inputs.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests, the struct, or the signatures.
//
//   1. `ParsedPath::parse`: `None` for an empty `path`; otherwise `Some`
//      with `full` set to the whole path and `last` set to its final
//      dot-separated segment (for a path with no dots, `last` equals `full`).
//   2. `common_prefix`: the longest run of whole leading segments shared by
//      `a` and `b`, returned as a slice *of `a`* (so the result only
//      borrows from `a` — that's why the signature ties the output
//      lifetime to `'a` and not to `b`'s lifetime). `""` if the first
//      segment already differs.
//   3. `is_descendant`: true if `child` is exactly `parent`, or `parent`
//      followed by `.` and more segments. `is_descendant("Vehicle.Cabin",
//      "Vehicle.CabinLight")` must be `false` — a naive `starts_with` isn't
//      enough, you need the dot boundary too.

struct ParsedPath<'a> {
    full: &'a str,
    last: &'a str,
}

impl<'a> ParsedPath<'a> {
    fn parse(path: &'a str) -> Option<Self> {
        if path.is_empty() {
            return None;
        }
        // WHY: rsplit + next gives the segment after the last '.', or the
        // whole string when there's no '.' at all — no allocation either way.
        let last = path.rsplit('.').next().unwrap_or(path);
        Some(ParsedPath { full: path, last })
    }
}

/// The longest run of whole leading segments shared by `a` and `b`, as a
/// slice of `a`.
fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut len = 0;
    let mut first = true;
    // WHY: comparing segment-by-segment (not char-by-char) means "Vehicle"
    // and "VehicleX" share zero segments, not a 7-char prefix.
    for (seg_a, seg_b) in a.split('.').zip(b.split('.')) {
        if seg_a != seg_b {
            break;
        }
        if !first {
            len += 1; // the '.' separator
        }
        len += seg_a.len();
        first = false;
    }
    &a[..len]
}

/// True if `child` is `parent` itself, or a dotted descendant of it.
fn is_descendant(parent: &str, child: &str) -> bool {
    child == parent
        || (child.starts_with(parent) && child.as_bytes().get(parent.len()) == Some(&b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_off_the_last_segment() {
        let parsed = ParsedPath::parse("Vehicle.Cabin.Door.Row1.Left").unwrap();
        assert_eq!(parsed.full, "Vehicle.Cabin.Door.Row1.Left");
        assert_eq!(parsed.last, "Left");
    }

    #[test]
    fn parse_with_no_dots_is_its_own_last_segment() {
        let parsed = ParsedPath::parse("Vehicle").unwrap();
        assert_eq!(parsed.last, "Vehicle");
    }

    #[test]
    fn parse_rejects_empty_path() {
        assert!(ParsedPath::parse("").is_none());
    }

    #[test]
    fn common_prefix_of_siblings() {
        assert_eq!(
            common_prefix(
                "Vehicle.Cabin.Door.Row1.Left.IsOpen",
                "Vehicle.Cabin.Door.Row1.Right.IsOpen"
            ),
            "Vehicle.Cabin.Door.Row1"
        );
    }

    #[test]
    fn common_prefix_of_identical_paths_is_the_whole_thing() {
        assert_eq!(
            common_prefix("Vehicle.Speed", "Vehicle.Speed"),
            "Vehicle.Speed"
        );
    }

    #[test]
    fn common_prefix_with_no_shared_segment_is_empty() {
        assert_eq!(common_prefix("Vehicle.Speed", "Cabin.Speed"), "");
    }

    #[test]
    fn is_descendant_covers_self_and_children() {
        assert!(is_descendant("Vehicle.Cabin", "Vehicle.Cabin"));
        assert!(is_descendant(
            "Vehicle.Cabin",
            "Vehicle.Cabin.Door.Row1.Left"
        ));
    }

    #[test]
    fn is_descendant_rejects_a_string_prefix_that_is_not_a_path_prefix() {
        assert!(!is_descendant("Vehicle.Cabin", "Vehicle.CabinLight"));
        assert!(!is_descendant("Vehicle.Cabin", "Vehicle.Trunk"));
    }
}
