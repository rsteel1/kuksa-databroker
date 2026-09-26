// c03_e06_validated_newtype.rs
//
// CONCEPT: newtypes and "make invalid states unrepresentable": private
//          fields + a validating constructor.
// GUIDE:   chapters/03-structs/guide.md  (section "Newtypes and validated construction")
// DATABROKER: databroker/src/types.rs — `pub struct SignalId { id: i32 }` wraps
//             an `i32` so a signal id can't be confused with any other number;
//             databroker/src/glob.rs — `is_valid_path` checks VSS paths.
// EXPECT:  tests fail: `todo!()` panics.
//
// A *newtype* is a struct with one field that gives a plain value a meaning:
// `SignalId(i32)` is not "just an int". If the field is private and the only
// way to build one is a constructor that validates, then *every* `VssPath`
// in the program is valid — you check once, at the border, and never again.
//
// Privacy in Rust is per *module*: code outside `mod vss` can't touch the
// private field, so it can't construct an invalid `VssPath` (try it in a
// test: `vss::VssPath(String::new())` is error E0603).
//
// YOUR TASK
//   Implement `SignalId::new`, `SignalId::id`, `VssPath::parse`,
//   `VssPath::as_str` and `VssPath::leaf`.
//   Do not edit the tests.

mod vss {
    /// A signal id. `Copy` because it's just a number.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SignalId(i32);

    impl SignalId {
        pub fn new(id: i32) -> Self {
            SignalId(id)
        }

        /// Note: `id()`, not `get_id()` — Rust getters don't use a `get_` prefix.
        pub fn id(&self) -> i32 {
            self.0
        }
    }

    /// A syntactically valid VSS path: one or more non-empty segments joined
    /// by '.', each segment made only of ASCII letters, digits and '_'.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct VssPath(String);

    impl VssPath {
        /// Returns `None` for invalid input.
        pub fn parse(input: &str) -> Option<VssPath> {
            if input.is_empty() {
                return None;
            }
            for segment in input.split('.') {
                if segment.is_empty() {
                    return None;
                }
                for c in segment.chars() {
                    if !(c.is_ascii_alphanumeric() || c == '_') {
                        return None;
                    }
                }
            }
            Some(VssPath(input.to_string()))
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }

        /// The last segment: "Vehicle.Speed" -> "Speed".
        pub fn leaf(&self) -> &str {
            // WHY: a valid path is never empty, so `split` always yields at
            // least one item; `expect` documents that invariant.
            self.0
                .split('.')
                .next_back()
                .expect("valid paths are non-empty")
        }
    }
}

use vss::{SignalId, VssPath};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_ids() {
        let id = SignalId::new(42);
        assert_eq!(id.id(), 42);
        let copy = id;
        assert_eq!(copy, id);
    }

    #[test]
    fn valid_paths() {
        let path = VssPath::parse("Vehicle.Cabin.Door.Row1.Left.IsOpen").expect("valid");
        assert_eq!(path.as_str(), "Vehicle.Cabin.Door.Row1.Left.IsOpen");
        assert_eq!(path.leaf(), "IsOpen");
        assert_eq!(
            VssPath::parse("Vehicle").map(|p| p.leaf().to_string()),
            Some("Vehicle".to_string())
        );
        assert!(VssPath::parse("Vehicle.Private_Thing2").is_some());
    }

    #[test]
    fn invalid_paths() {
        assert_eq!(VssPath::parse(""), None);
        assert_eq!(VssPath::parse("Vehicle..Speed"), None);
        assert_eq!(VssPath::parse(".Vehicle"), None);
        assert_eq!(VssPath::parse("Vehicle."), None);
        assert_eq!(VssPath::parse("Vehicle.Sp eed"), None);
        assert_eq!(VssPath::parse("Vehicle.*"), None);
    }
}
