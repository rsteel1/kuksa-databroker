// c29_e01_string_methods.rs
//
// CONCEPT: everyday `&str` methods, and returning slices that borrow the input.
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "Working with VSS paths as &str")
// DATABROKER: databroker/src/glob.rs — `Matcher::to_glob_string` (`replace('.', "/")`),
//             `Matcher::is_match` (`strip_suffix("/**")`)
// EXPECT:  tests fail: `todo!()` panics
//
// Signal paths are strings like "Vehicle.Cabin.Door.Row1.IsOpen". The
// databroker constantly splits, joins and rewrites them. None of the
// functions below needs to allocate unless it builds a *new* string:
// `parent` and `leaf` can return a slice of the input — the lifetime
// elision rules tie the returned `&str` to the `path` argument.
//
// YOUR TASK
//   Implement every function. Prefer std methods over manual char loops:
//   `split`, `rsplit_once`, `strip_suffix`, `replace`, `join`, `is_empty`.
//   Do not edit the tests.

/// Number of segments: "Vehicle.Speed" -> 2. The empty string has 0.
fn depth(path: &str) -> usize {
    if path.is_empty() {
        0
    } else {
        path.split('.').count()
    }
}

/// "Vehicle.Cabin.Door" -> Some("Vehicle.Cabin"); "Vehicle" -> None.
fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(parent, _leaf)| parent)
}

/// "Vehicle.Cabin.Door" -> "Door"; "Vehicle" -> "Vehicle".
fn leaf(path: &str) -> &str {
    path.rsplit_once('.').map_or(path, |(_parent, leaf)| leaf)
}

/// ["Vehicle", "Cabin"] -> "Vehicle.Cabin"
fn join_path(segments: &[&str]) -> String {
    segments.join(".")
}

/// The glob library the databroker uses expects '/' separators:
/// "Vehicle.Cabin.*" -> "Vehicle/Cabin/*"
fn to_slash_path(path: &str) -> String {
    path.replace('.', "/")
}

/// "Vehicle/Cabin/**" -> Some("Vehicle/Cabin"); anything else -> None.
fn branch_of_recursive_glob(glob: &str) -> Option<&str> {
    glob.strip_suffix("/**")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_counts_segments() {
        assert_eq!(depth("Vehicle.Cabin.Door.Row1.IsOpen"), 5);
        assert_eq!(depth("Vehicle"), 1);
        assert_eq!(depth(""), 0);
    }

    #[test]
    fn parent_and_leaf() {
        assert_eq!(parent("Vehicle.Cabin.Door"), Some("Vehicle.Cabin"));
        assert_eq!(parent("Vehicle"), None);
        assert_eq!(leaf("Vehicle.Cabin.Door"), "Door");
        assert_eq!(leaf("Vehicle"), "Vehicle");
    }

    #[test]
    fn parent_borrows_from_the_input() {
        let owned = String::from("Vehicle.Speed");
        let p = parent(&owned).unwrap();
        // `p` points into `owned`'s buffer: same address, no allocation.
        assert_eq!(p.as_ptr(), owned.as_ptr());
    }

    #[test]
    fn joining_and_rewriting() {
        assert_eq!(
            join_path(&["Vehicle", "Cabin", "Door"]),
            "Vehicle.Cabin.Door"
        );
        assert_eq!(join_path(&[]), "");
        assert_eq!(to_slash_path("Vehicle.Cabin.*"), "Vehicle/Cabin/*");
    }

    #[test]
    fn recursive_glob_suffix() {
        assert_eq!(
            branch_of_recursive_glob("Vehicle/Cabin/**"),
            Some("Vehicle/Cabin")
        );
        assert_eq!(branch_of_recursive_glob("Vehicle/Cabin/*"), None);
        assert_eq!(branch_of_recursive_glob("**"), None);
    }
}
