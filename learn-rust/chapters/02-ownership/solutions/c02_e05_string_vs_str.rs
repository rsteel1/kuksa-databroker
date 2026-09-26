// c02_e05_string_vs_str.rs
//
// CONCEPT: `String` (owned, growable) vs `&str` (borrowed view of text).
// GUIDE:   chapters/02-ownership/guide.md  (section "String vs &str")
// DATABROKER: databroker/src/broker.rs — `get_metadata_by_path(&self, path:
//             &str)`: lookups only need to read the path, so they take `&str`.
// EXPECT:  compile error E0308 (expected `String`, found `&str`).
//
// Rule of thumb: take `&str` when you only read text; return / store `String`
// when you create or keep text. A `&String` converts to `&str` for free
// (deref coercion), and so does a string literal, so `&str` parameters accept
// everything.
//
// VSS paths look like "Vehicle.Cabin.Door.Row1.Left.IsOpen": segments
// separated by dots. The last segment is the "leaf", the rest is the "branch".
//
// YOUR TASK
//   1. Fix the signature of `is_vehicle_signal`.
//   2. Implement `leaf_name` and `branch` — both return a *slice of the input*
//      (no allocation). Look at `str::rsplit_once`.
//   3. Implement `join_path`, which must build a new `String`.
//   Do not edit the tests.

fn is_vehicle_signal(path: &str) -> bool {
    path.starts_with("Vehicle.")
}

/// "Vehicle.Cabin.Temperature" -> "Temperature"; "Vehicle" -> "Vehicle".
fn leaf_name(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((_, leaf)) => leaf,
        None => path,
    }
}

/// "Vehicle.Cabin.Temperature" -> "Vehicle.Cabin"; "Vehicle" -> "".
fn branch(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((branch, _)) => branch,
        None => "",
    }
}

/// ("Vehicle.Cabin", "Temperature") -> "Vehicle.Cabin.Temperature".
/// An empty branch gives just the leaf.
fn join_path(branch: &str, leaf: &str) -> String {
    if branch.is_empty() {
        leaf.to_string()
    } else {
        format!("{branch}.{leaf}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vehicle_signals() {
        assert!(is_vehicle_signal("Vehicle.Speed"));
        let owned = String::from("Private.Thing");
        assert!(!is_vehicle_signal(&owned));
    }

    #[test]
    fn leaves_and_branches() {
        assert_eq!(leaf_name("Vehicle.Cabin.Temperature"), "Temperature");
        assert_eq!(branch("Vehicle.Cabin.Temperature"), "Vehicle.Cabin");
        assert_eq!(leaf_name("Vehicle"), "Vehicle");
        assert_eq!(branch("Vehicle"), "");
    }

    #[test]
    fn joining() {
        assert_eq!(
            join_path("Vehicle.Cabin", "Temperature"),
            "Vehicle.Cabin.Temperature"
        );
        assert_eq!(join_path("", "Vehicle"), "Vehicle");
        let path = String::from("Vehicle.Speed");
        assert_eq!(join_path(branch(&path), leaf_name(&path)), path);
    }
}
