// c05_e04_btreemap_sorted.rs
//
// CONCEPT: BTreeMap<K, V> — a map that keeps keys sorted; range queries.
// GUIDE:   chapters/05-collections/guide.md  (section "BTreeMap: sorted keys")
// DATABROKER: databroker/src/vss.rs — `parse_vss_from_str` returns
//             `BTreeMap<String, DataEntry>`, so signals are registered in a
//             stable, alphabetical order every time the broker starts.
// EXPECT:  tests fail: `todo!()` panics
//
// Because a BTreeMap is sorted, all keys that start with "Vehicle.Cabin."
// sit next to each other. `map.range(start..)` jumps straight to the first
// key >= start; from there you can walk forward while the prefix matches.
//
// YOUR TASK
//   Implement the `todo!()`s. `children_of` must use `range` (not a scan
//   over the whole map). Do not edit the tests.

// I AM NOT DONE

use std::collections::BTreeMap;

/// Builds a path -> unit map. Later duplicates overwrite earlier ones.
fn build(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    todo!()
}

/// All paths strictly below `branch` (i.e. starting with "branch."), in order.
/// The `'m` annotation says "the returned &strs borrow from `map`" (not from
/// `branch`). Lifetimes get a whole chapter later (10); just read it for now.
fn children_of<'m>(map: &'m BTreeMap<String, String>, branch: &str) -> Vec<&'m str> {
    todo!()
}

/// The alphabetically first and last paths, or None when empty.
fn first_and_last(map: &BTreeMap<String, String>) -> Option<(&str, &str)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BTreeMap<String, String> {
        build(&[
            ("Vehicle.Speed", "km/h"),
            ("Vehicle.Cabin.Door.Row1.Left.IsOpen", ""),
            ("Vehicle.Cabin.Temperature", "celsius"),
            ("Vehicle.CabinX.Weird", ""),
            ("Vehicle.Body.Lights.IsOn", ""),
            ("Vehicle.Speed", "m/s"),
        ])
    }

    #[test]
    fn keys_are_sorted_and_deduplicated() {
        let map = sample();
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "Vehicle.Body.Lights.IsOn",
                "Vehicle.Cabin.Door.Row1.Left.IsOpen",
                "Vehicle.Cabin.Temperature",
                "Vehicle.CabinX.Weird",
                "Vehicle.Speed",
            ]
        );
        assert_eq!(map["Vehicle.Speed"], "m/s", "later entry wins");
    }

    #[test]
    fn children_uses_prefix_with_dot() {
        let map = sample();
        assert_eq!(
            children_of(&map, "Vehicle.Cabin"),
            vec![
                "Vehicle.Cabin.Door.Row1.Left.IsOpen",
                "Vehicle.Cabin.Temperature"
            ]
        );
        assert!(children_of(&map, "Vehicle.Speed").is_empty());
        assert_eq!(children_of(&map, "Vehicle").len(), 5);
    }

    #[test]
    fn first_last() {
        assert_eq!(
            first_and_last(&sample()),
            Some(("Vehicle.Body.Lights.IsOn", "Vehicle.Speed"))
        );
        assert_eq!(first_and_last(&BTreeMap::new()), None);
    }
}
