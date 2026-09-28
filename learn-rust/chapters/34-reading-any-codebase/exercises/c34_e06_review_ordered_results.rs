// c34_e06_review_ordered_results.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs in code
//          that leans on `IndexMap`'s one distinguishing feature (it
//          remembers insertion order) and then throws that feature away:
//          a "collect the answers" loop that walks the request list
//          backwards, and a "remove what's done" loop that uses the wrong
//          one of `swap_remove`/`shift_remove`.
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Reading
//          data-structure code")
// DATABROKER: databroker/src/broker.rs:229 — `pub entries: IndexMap<SignalId, Datapoint>`
//             on the response type exists so results come back in the SAME
//             order the client asked for them; databroker/src/broker.rs:2200-2204
//             — `get_values_broker` builds that map by walking the
//             requested ids in order, not by walking whatever a provider's
//             `HashMap` happened to iterate in. `indexmap` (2.14.2, under
//             `~/.cargo/registry/src/index.crates.io-*/indexmap-2.14.2/`)
//             src/inner.rs:564 `shift_remove_finish` (O(n), keeps order) vs
//             src/inner.rs:587 `swap_remove_finish` (O(1), moves the last
//             entry into the hole — reorders).
// EXPECT:  tests fail: asking for ids `[3, 1, 2]` returns them as
//          `[2, 1, 3]`, and removing an answered id from the middle of a
//          pending list silently reorders the ids still waiting.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests.

// I AM NOT DONE

use indexmap::IndexMap;
use std::collections::HashMap;

/// Mirrors the shape of `Database::get_values_broker`: for each requested
/// id, prefer a provider's `fresh` answer, falling back to `db`; ids with
/// neither are simply absent from the result. The whole point of returning
/// an `IndexMap` instead of a `HashMap` is that the RESULT comes back in
/// the same order as `ids`, so callers can zip it back up with their
/// original request.
fn collect_values(
    ids: &[i32],
    fresh: &HashMap<i32, f32>,
    db: &HashMap<i32, f32>,
) -> IndexMap<i32, f32> {
    let mut result = IndexMap::new();
    for id in ids.iter().rev() {
        if let Some(value) = fresh.get(id).or_else(|| db.get(id)) {
            result.insert(*id, *value);
        }
    }
    result
}

/// Removes every id in `answered` from `pending`. The ids still waiting
/// afterwards must stay in their original relative order — callers report
/// "still waiting on: ..." in that order, and a reordering would make the
/// wait times reported alongside them line up with the wrong ids.
fn drop_answered(pending: &mut IndexMap<i32, ()>, answered: &[i32]) {
    for id in answered {
        pending.swap_remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_request_order() {
        let fresh = HashMap::from([(1, 1.5), (3, 3.5)]);
        let db = HashMap::from([(2, 2.5)]);
        let values = collect_values(&[3, 1, 2], &fresh, &db);
        assert_eq!(
            values.keys().copied().collect::<Vec<_>>(),
            vec![3, 1, 2],
            "results must come back in the order they were requested in"
        );
        assert_eq!(values[&3], 3.5);
        assert_eq!(values[&2], 2.5);
    }

    #[test]
    fn an_id_with_no_answer_is_simply_absent() {
        let fresh = HashMap::new();
        let db = HashMap::from([(1, 1.5)]);
        let values = collect_values(&[1, 999], &fresh, &db);
        assert_eq!(values.keys().copied().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn removing_an_answered_id_keeps_the_rest_in_order() {
        let mut pending: IndexMap<i32, ()> =
            [10, 20, 30, 40].into_iter().map(|id| (id, ())).collect();
        drop_answered(&mut pending, &[20]);
        assert_eq!(
            pending.keys().copied().collect::<Vec<_>>(),
            vec![10, 30, 40],
            "removing id 20 must not reorder the ids still pending"
        );
    }
}
