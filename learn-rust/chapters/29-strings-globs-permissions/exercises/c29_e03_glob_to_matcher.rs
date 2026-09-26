// c29_e03_glob_to_matcher.rs
//
// CONCEPT: a validated wrapper type around a glob pattern (construct once, match many times).
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "VSS wildcards and the Matcher type")
// DATABROKER: databroker/src/glob.rs — `Matcher::new`, `Matcher::to_glob_string`,
//             `Matcher::is_match`; doc/wildcard_matching.md
// EXPECT:  tests fail: `todo!()` panics
//
// Clients may ask for "Vehicle.Cabin.*" or "Vehicle.**.IsOpen" or just
// "Vehicle.Cabin" (meaning: that signal, OR everything below that branch).
// The databroker translates the VSS pattern into the syntax of the
// `glob-match` crate once, in `Matcher::new`, and keeps only the translated
// string. Invalid patterns are rejected at construction, so a `Matcher` in
// hand is always valid ("parse, don't validate").
//
// Translation rules (same as the databroker):
//   * "" (or the two-character string `""`) means "**" — everything
//   * '.' becomes '/'
//   * if the pattern has no '*' at all, append "/**" so a branch name also
//     matches all its children
//   * `is_match`: a `**` at the end should also match the branch itself:
//     "Vehicle/Cabin/**" must match "Vehicle.Cabin". The databroker handles
//     this by retrying without the "/**" suffix.
//   * `is_match` receives dotted VSS paths; convert before calling glob_match.
//   * A pattern is valid when every '.'-separated segment is non-empty and has
//     no ':' and no whitespace ('*' is allowed).
//
// YOUR TASK
//   Implement `Matcher::new`, `Matcher::is_match` and `Matcher::as_glob`.
//   Do not edit the tests.

// I AM NOT DONE

use glob_match::glob_match;

#[derive(Debug, PartialEq)]
struct InvalidPattern;

#[derive(Debug)]
struct Matcher {
    glob: String,
}

fn is_valid_pattern(pattern: &str) -> bool {
    pattern
        .split('.')
        .all(|seg| !seg.is_empty() && !seg.chars().any(|c| c == ':' || c.is_whitespace()))
}

impl Matcher {
    fn new(pattern: &str) -> Result<Self, InvalidPattern> {
        todo!()
    }

    fn is_match(&self, vss_path: &str) -> bool {
        todo!()
    }

    fn as_glob(&self) -> &str {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str) -> Matcher {
        Matcher::new(p).expect("valid pattern")
    }

    #[test]
    fn translation() {
        assert_eq!(m("Vehicle.Speed").as_glob(), "Vehicle/Speed/**");
        assert_eq!(m("Vehicle.Cabin.*").as_glob(), "Vehicle/Cabin/*");
        assert_eq!(m("Vehicle.**.IsOpen").as_glob(), "Vehicle/**/IsOpen");
        assert_eq!(m("").as_glob(), "**");
        assert_eq!(m("\"\"").as_glob(), "**");
    }

    #[test]
    fn invalid_patterns_are_rejected() {
        assert_eq!(Matcher::new("Vehicle..Speed").unwrap_err(), InvalidPattern);
        assert_eq!(Matcher::new("Vehicle.").unwrap_err(), InvalidPattern);
        assert_eq!(Matcher::new("Vehicle:Speed").unwrap_err(), InvalidPattern);
        assert_eq!(Matcher::new("Vehicle Speed").unwrap_err(), InvalidPattern);
    }

    #[test]
    fn exact_signal_and_branch() {
        let speed = m("Vehicle.Speed");
        assert!(speed.is_match("Vehicle.Speed"));
        assert!(!speed.is_match("Vehicle.SpeedLimit"), "no prefix matching!");

        let cabin = m("Vehicle.Cabin");
        assert!(cabin.is_match("Vehicle.Cabin"));
        assert!(cabin.is_match("Vehicle.Cabin.Door.Row1.IsOpen"));
        assert!(!cabin.is_match("Vehicle.Speed"));
    }

    #[test]
    fn single_star_is_one_level() {
        let cabin_children = m("Vehicle.Cabin.*");
        assert!(cabin_children.is_match("Vehicle.Cabin.Door"));
        assert!(!cabin_children.is_match("Vehicle.Cabin.Door.Row1"));
        assert!(!cabin_children.is_match("Vehicle.Cabin"));
    }

    #[test]
    fn double_star_is_any_depth() {
        let is_open = m("Vehicle.**.IsOpen");
        assert!(is_open.is_match("Vehicle.Cabin.Door.Row1.IsOpen"));
        assert!(is_open.is_match("Vehicle.Trunk.IsOpen"));
        assert!(!is_open.is_match("Vehicle.Trunk.IsLocked"));

        let everything = m("");
        assert!(everything.is_match("Vehicle.Speed"));
        assert!(everything.is_match("Kuksa.Databroker.CargoVersion"));
    }
}
