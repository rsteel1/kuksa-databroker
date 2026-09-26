// c14_e05_deref_asref_borrow.rs
//
// CONCEPT: the three "view me as something else" traits:
//          `Deref` (auto-deref + method lookup), `AsRef` (cheap explicit
//          conversion for generic params), `Borrow` (a promise that Hash/Eq/Ord
//          agree — what lets `HashMap<Owned, V>` be queried with `&Borrowed`).
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Deref, AsRef and Borrow")
// DATABROKER: databroker/src/broker.rs — `DatabaseReadAccess::get_entry_by_path(path: impl AsRef<str>)`;
//             `Database::path_to_id: HashMap<String, i32>` is queried with a
//             `&str` — that works because `String: Borrow<str>`.
// EXPECT:  several compile errors at once: E0308 twice (`&VssPath` where a
//          `&str` is expected, and `map.get(path)` wanting a `&VssPath`),
//          E0599 (no method named `starts_with` for `VssPath`) and E0277
//          (`VssPath: AsRef<str>` not satisfied)
//
// `VssPath` is a newtype around `String` (chapter 03 style). Right now it is
// awkward: you can't call `str` methods on it, can't pass it where
// `impl AsRef<str>` is expected, and a `HashMap<VssPath, i32>` can only be
// queried by building a whole `VssPath` (an allocation per lookup!).
//
// YOUR TASK
//   Implement `Deref<Target = str>`, `AsRef<str>` and `Borrow<str>` for
//   `VssPath`. Then `lookup` below compiles as written, with no allocation.
//   Do not edit the tests or `lookup`/`byte_len`.

// I AM NOT DONE

use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Deref;

/// Hash and Eq are derived from the inner String, so they give exactly the
/// same answers as `str`'s — the promise `Borrow<str>` requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VssPath(String);

// TODO: impl Deref for VssPath (Target = str)
// TODO: impl AsRef<str> for VssPath
// TODO: impl Borrow<str> for VssPath

/// Query a map keyed by `VssPath` with a plain `&str` — no allocation.
fn lookup(map: &HashMap<VssPath, i32>, path: &str) -> Option<i32> {
    map.get(path).copied()
}

/// Works for &str, String and VssPath alike.
fn byte_len(path: impl AsRef<str>) -> usize {
    path.as_ref().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deref_gives_str_methods() {
        let p = VssPath("Vehicle.Speed".to_string());
        assert!(p.starts_with("Vehicle."));
        assert_eq!(p.split('.').count(), 2);
        let s: &str = &p; // deref coercion: &VssPath -> &str
        assert_eq!(s, "Vehicle.Speed");
    }

    #[test]
    fn as_ref_accepts_all_string_kinds() {
        assert_eq!(byte_len("abc"), 3);
        assert_eq!(byte_len(String::from("abcd")), 4);
        assert_eq!(byte_len(VssPath("Vehicle".into())), 7);
    }

    #[test]
    fn borrow_allows_str_lookup() {
        let mut map = HashMap::new();
        map.insert(VssPath("Vehicle.Speed".into()), 7);
        assert_eq!(lookup(&map, "Vehicle.Speed"), Some(7));
        assert_eq!(lookup(&map, "Vehicle.Rpm"), None);
    }
}
