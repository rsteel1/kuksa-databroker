// c05_e03_hashset_ops.rs
//
// CONCEPT: HashSet<T> and set algebra: is_disjoint, intersection, is_subset.
// GUIDE:   chapters/05-collections/guide.md  (section "HashSet and set algebra")
// DATABROKER: databroker/src/broker.rs — `ChangeSubscription::notify` uses
//             `fields.is_disjoint(changed_fields)`; `register_signals` uses
//             `intersection` to reject already-provided signals;
//             `valid_provider_publish_signals` uses `is_subset`.
// EXPECT:  tests fail: `todo!()` panics
//
// A subscription says "for signal 7 I care about the Datapoint field".
// An update says "signal 7 changed its ActuatorTarget". Do they overlap?
// That question is exactly "are these two sets NOT disjoint?".
//
// YOUR TASK
//   Implement the three functions with set operations — no nested loops
//   comparing elements by hand. Do not edit the tests.

// I AM NOT DONE

use std::collections::{HashMap, HashSet};

/// Which part of an entry changed. Deriving `Hash` + `Eq` is what allows
/// this type to live inside a HashSet (the real `broker::Field` does the same).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Field {
    Datapoint,
    ActuatorTarget,
    MetadataUnit,
}

/// Does any changed (id, field) pair appear in the subscription?
/// `subscribed`: id -> fields the subscriber wants.
/// `changed`:    id -> fields that changed.
fn subscription_matches(
    subscribed: &HashMap<i32, HashSet<Field>>,
    changed: &HashMap<i32, HashSet<Field>>,
) -> bool {
    todo!()
}

/// Signals a new provider asks for that some other provider already owns,
/// sorted ascending (HashSet iteration order is random!).
fn already_provided(owned: &HashSet<i32>, requested: &HashSet<i32>) -> Vec<i32> {
    todo!()
}

/// A provider may only publish signals it registered for.
fn may_publish(registered: &HashSet<i32>, publishing: &HashSet<i32>) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<T: std::hash::Hash + Eq + Clone>(items: &[T]) -> HashSet<T> {
        items.iter().cloned().collect()
    }

    #[test]
    fn matches_when_same_id_and_overlapping_field() {
        let subscribed = HashMap::from([(7, set(&[Field::Datapoint]))]);
        let changed = HashMap::from([(7, set(&[Field::Datapoint, Field::MetadataUnit]))]);
        assert!(subscription_matches(&subscribed, &changed));
    }

    #[test]
    fn no_match_when_fields_differ() {
        let subscribed = HashMap::from([(7, set(&[Field::Datapoint]))]);
        let changed = HashMap::from([(7, set(&[Field::ActuatorTarget]))]);
        assert!(!subscription_matches(&subscribed, &changed));
    }

    #[test]
    fn no_match_for_other_ids() {
        let subscribed = HashMap::from([(1, set(&[Field::Datapoint]))]);
        let changed = HashMap::from([(2, set(&[Field::Datapoint]))]);
        assert!(!subscription_matches(&subscribed, &changed));
        assert!(!subscription_matches(&subscribed, &HashMap::new()));
    }

    #[test]
    fn finds_clashes_sorted() {
        let owned = set(&[1, 2, 3, 10]);
        let requested = set(&[10, 3, 42]);
        assert_eq!(already_provided(&owned, &requested), vec![3, 10]);
        assert!(already_provided(&owned, &set(&[99])).is_empty());
    }

    #[test]
    fn subset_rule() {
        let registered = set(&[1, 2, 3]);
        assert!(may_publish(&registered, &set(&[1, 3])));
        assert!(may_publish(&registered, &HashSet::new()));
        assert!(!may_publish(&registered, &set(&[3, 4])));
    }
}
