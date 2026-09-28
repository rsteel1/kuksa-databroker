// c02_e07_build_path_buffer.rs
//
// CONCEPT: writing several small functions from a spec: move vs. borrow,
//          `&str` vs `String`, and slices, on a buffer of VSS paths.
// GUIDE:   chapters/02-ownership/guide.md  (section "Borrowing: lending instead of giving")
// DATABROKER: databroker/src/glob.rs — path handling that borrows `&str`
//             wherever possible instead of taking ownership.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. Implement each function so every test passes. Think about
// which functions need to *own* their data and which can just *borrow* it.
//
// YOUR TASK
//   Implement `push_unique`, `common_prefix`, `dedup_paths` and
//   `take_first` below.
//   Do not edit the tests.

// I AM NOT DONE

/// Pushes `path` onto `paths` if it isn't already there. Returns whether
/// it was inserted.
fn push_unique(paths: &mut Vec<String>, path: &str) -> bool {
    if paths.contains(&path.to_string()) { 
        false
    } else {
        paths.push(path.to_string());
        true
    }
}

/// The longest prefix of `a` that is also a prefix of `b`, only ever
/// splitting on a full `.`-separated segment (never inside one).
/// `common_prefix("Vehicle.Cabin.Door", "Vehicle.Cabin.Light")` is
/// `"Vehicle.Cabin"`. No shared segment at all means `""`.
fn common_prefix<'a>(a: &'a str, b: &'a str) -> &'a str {
    let a_split = a.split(".");
    let b_split = b.split(".");
    let zipped_slice = a_split.zip(b_split);
    let mut size: usize = 0;
    for (a_char, b_char) in zipped_slice {
        if a_char != b_char {
            break;
        } else {
            size += 1
        }
    }
    &a[0..size]
}

/// Returns a new `Vec` with each distinct path from `paths`, keeping the
/// first occurrence of each and its original order.
fn dedup_paths(paths: &[String]) -> Vec<String> {
    todo!()
}

/// A sub-slice of the first `n` paths, or every path if `n` is larger
/// than `paths.len()`.
fn take_first(paths: &[String], n: usize) -> &[String] {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_unique_dedups() {
        let mut paths = vec!["Vehicle.Speed".to_string()];
        assert!(!push_unique(&mut paths, "Vehicle.Speed"));
        assert!(push_unique(&mut paths, "Vehicle.Cabin.Lights"));
        assert_eq!(
            paths,
            vec![
                "Vehicle.Speed".to_string(),
                "Vehicle.Cabin.Lights".to_string()
            ]
        );
    }

    #[test]
    fn common_prefixes() {
        assert_eq!(
            common_prefix(
                "Vehicle.Cabin.Door.Row1.Left",
                "Vehicle.Cabin.Door.Row2.Left"
            ),
            "Vehicle.Cabin.Door"
        );
        assert_eq!(
            common_prefix("Vehicle.Speed", "Vehicle.Speed"),
            "Vehicle.Speed"
        );
        assert_eq!(common_prefix("Vehicle.Speed", "Engine.Speed"), "");
    }

    #[test]
    fn dedup_preserves_order() {
        let paths: Vec<String> = ["A", "B", "A", "C", "B"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let want: Vec<String> = ["A", "B", "C"].iter().map(|s| s.to_string()).collect();
        assert_eq!(dedup_paths(&paths), want);
    }

    #[test]
    fn take_first_clamps() {
        let paths: Vec<String> = ["A", "B"].iter().map(|s| s.to_string()).collect();
        assert_eq!(take_first(&paths, 1), &[paths[0].clone()]);
        assert_eq!(take_first(&paths, 10), &paths[..]);
    }
}
