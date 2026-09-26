// c10_e06_subscription_matching.rs
//
// CONCEPT: consuming adaptors — `any`, `all`, `fold`, `find`, `chain` — to
//          express "does any of these match?" logic without flags and loops.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Consumers: any, all, fold, find")
// DATABROKER: databroker/src/broker.rs — `ChangeSubscription::notify` decides
//          whether a subscription cares about a batch of changes with a
//          `for` loop, a `matches` flag and `is_disjoint` (around line 936).
// EXPECT:  tests fail: `todo!()` panics.
//
// When the databroker updates some signals it knows, per signal id, WHICH
// fields changed (value, actuator target, ...). Each subscription knows, per
// signal id, which fields it wants. A subscription must be notified if for
// *any* changed id it subscribed to, the two field sets overlap.
//
// YOUR TASK
//   Implement the functions with iterator consumers (no `for`, no `mut`
//   flags). Do not edit the tests.

// I AM NOT DONE

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Field {
    Datapoint,
    ActuatorTarget,
}

type FieldsById = HashMap<i32, HashSet<Field>>;

/// True if, for any id present in both maps, the field sets overlap.
fn subscription_matches(subscribed: &FieldsById, changed: &FieldsById) -> bool {
    todo!()
}

/// True if every path is non-empty and contains no whitespace.
fn all_paths_valid(paths: &[&str]) -> bool {
    todo!()
}

/// Total number of characters of all paths, using `fold`.
fn total_path_len(paths: &[&str]) -> usize {
    todo!()
}

/// The first path (from `primary`, then `fallback`) that starts with `prefix`.
/// Use `chain` + `find`.
// `'a` says the returned `&str` borrows from the input slices, not from
// `prefix`. That's a named lifetime, which chapter 11 explains; you don't need to change it.
fn first_with_prefix<'a>(
    primary: &[&'a str],
    fallback: &[&'a str],
    prefix: &str,
) -> Option<&'a str> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(f: &[Field]) -> HashSet<Field> {
        f.iter().copied().collect()
    }

    #[test]
    fn matches_when_fields_overlap() {
        let subscribed = HashMap::from([
            (1, fields(&[Field::Datapoint])),
            (2, fields(&[Field::ActuatorTarget])),
        ]);
        let changed = HashMap::from([(2, fields(&[Field::ActuatorTarget, Field::Datapoint]))]);
        assert!(subscription_matches(&subscribed, &changed));
    }

    #[test]
    fn no_match_when_fields_disjoint_or_ids_differ() {
        let subscribed = HashMap::from([(1, fields(&[Field::Datapoint]))]);
        let other_field = HashMap::from([(1, fields(&[Field::ActuatorTarget]))]);
        let other_id = HashMap::from([(7, fields(&[Field::Datapoint]))]);
        assert!(!subscription_matches(&subscribed, &other_field));
        assert!(!subscription_matches(&subscribed, &other_id));
        assert!(!subscription_matches(&subscribed, &HashMap::new()));
    }

    #[test]
    fn validates_paths() {
        assert!(all_paths_valid(&["Vehicle.Speed", "A.B"]));
        assert!(!all_paths_valid(&["Vehicle.Speed", ""]));
        assert!(!all_paths_valid(&["Vehicle Speed"]));
        assert!(all_paths_valid(&[]));
    }

    #[test]
    fn sums_lengths() {
        assert_eq!(total_path_len(&["ab", "cde", ""]), 5);
    }

    #[test]
    fn finds_across_both_lists() {
        assert_eq!(
            first_with_prefix(&["A.x", "B.y"], &["C.z"], "B."),
            Some("B.y")
        );
        assert_eq!(first_with_prefix(&["A.x"], &["C.z"], "C."), Some("C.z"));
        assert_eq!(first_with_prefix(&["A.x"], &["C.z"], "D."), None);
    }
}
