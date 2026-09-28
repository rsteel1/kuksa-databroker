// c05_e08_review_load_cleanup.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 05 (an inverted `retain`
//          predicate, a value read before the mutation it's supposed to
//          reflect, an off-by-one slice bound).
// GUIDE:   chapters/05-collections/guide.md  (section "Removing while iterating: retain")
// DATABROKER: databroker/src/broker.rs — `Subscriptions::cleanup` removes
//             dead subscriptions with `retain`; an inverted predicate
//             (bug 1 here) would keep every dead one and evict every
//             live one instead.
// EXPECT:  tests fail: cleanup keeps the wrong entries, a subscriber
//          count is stale, and the top-N list is missing an entry.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

use std::collections::{BTreeMap, HashMap, HashSet};

/// Removes every id whose receiver count is `0` (nobody is listening
/// anymore). Returns how many were removed.
fn cleanup_dead_receivers(receiver_counts: &mut HashMap<u32, usize>) -> usize {
    let before = receiver_counts.len();
    // BUG 1: `retain` KEEPS entries where the predicate is true, so
    // `*count == 0` kept the dead receivers and evicted the live ones —
    // the exact opposite of the doc comment. Keep the live ones instead.
    receiver_counts.retain(|_, count| *count != 0);
    before - receiver_counts.len()
}

/// Subscribes `id` to `path`, returning how many subscribers `path` has
/// *after* this subscription is added.
fn subscribe_and_count(index: &mut HashMap<String, HashSet<u32>>, path: &str, id: u32) -> usize {
    // BUG 2: `existing` was captured before the insert and returned
    // as-is, so the count was always one short (the doc comment asks
    // for the count *after* adding). Read the length again once the
    // insert has happened.
    index.entry(path.to_string()).or_default().insert(id);
    index.get(path).map(|s| s.len()).unwrap_or(0)
}

/// The `n` paths with the most subscribers, highest first, ties broken
/// alphabetically. Fewer than `n` paths in `load` is not an error.
fn top_n_loaded(load: &BTreeMap<String, usize>, n: usize) -> Vec<(String, usize)> {
    let mut entries: Vec<(String, usize)> = load.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    // BUG 3: `..n - 1` dropped the nth entry (and panics if `n` is 0);
    // clamp to what's available and take exactly that many.
    let take_n = n.min(entries.len());
    entries[..take_n].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_dead_receivers_removes_only_dead_ones() {
        let mut counts = HashMap::from([(1, 0), (2, 3), (3, 0)]);
        let removed = cleanup_dead_receivers(&mut counts);
        assert_eq!(removed, 2);
        assert_eq!(counts, HashMap::from([(2, 3)]));
    }

    #[test]
    fn subscribe_and_count_reports_the_count_after_adding() {
        let mut index: HashMap<String, HashSet<u32>> = HashMap::new();
        assert_eq!(subscribe_and_count(&mut index, "Vehicle.Speed", 1), 1);
        assert_eq!(subscribe_and_count(&mut index, "Vehicle.Speed", 2), 2);
    }

    #[test]
    fn top_n_loaded_returns_exactly_n_entries() {
        let load = BTreeMap::from([
            ("Vehicle.Speed".to_string(), 5),
            ("Vehicle.RPM".to_string(), 2),
            ("Vehicle.Gear".to_string(), 8),
        ]);
        assert_eq!(
            top_n_loaded(&load, 2),
            vec![
                ("Vehicle.Gear".to_string(), 8),
                ("Vehicle.Speed".to_string(), 5)
            ]
        );
    }
}
