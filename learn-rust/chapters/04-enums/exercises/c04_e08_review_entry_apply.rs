// c04_e08_review_entry_apply.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 04 (a wildcard match arm
//          that swallows a case it shouldn't, an off-by-one loop bound,
//          a shadowed accumulator inside a match arm).
// GUIDE:   chapters/04-enums/guide.md  (section "Exhaustive matching and the wildcard trap")
// DATABROKER: databroker/src/broker.rs — `Entry::apply` applies a
//             tri-state `Option<Option<T>>` update; lumping `Some(None)`
//             ("clear it") into the same wildcard arm as `None`
//             ("leave it") is exactly bug 1 here.
// EXPECT:  tests fail: a clear doesn't take effect, the latest value is
//          wrong, and the update/clear counts are wrong.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChangeType {
    NoChange,
    Cleared,
    Updated(f64),
}

/// Applies a tri-state update to `current`:
///   `None`             -> leave `current` as it is             -> NoChange
///   `Some(None)`       -> clear `current` to `None`             -> Cleared
///   `Some(Some(v))`    -> set `current` to `v`                  -> Updated(v)
fn apply(current: &mut Option<f64>, update: Option<Option<f64>>) -> ChangeType {
    match update {
        None => ChangeType::NoChange,
        Some(Some(v)) => {
            *current = Some(v);
            ChangeType::Updated(v)
        }
        _ => ChangeType::NoChange,
    }
}

/// The value from the most recent `Updated`, or `None` if the most
/// recent relevant change was a `Cleared` (or there were none).
fn latest_value(changes: &[ChangeType]) -> Option<f64> {
    let mut latest = None;
    for i in 0..changes.len().saturating_sub(1) {
        match &changes[i] {
            ChangeType::Updated(v) => latest = Some(*v),
            ChangeType::Cleared => latest = None,
            ChangeType::NoChange => {}
        }
    }
    latest
}

/// Returns `(number of Updated, number of Cleared)` in `changes`.
fn describe_changes(changes: &[ChangeType]) -> (usize, usize) {
    let mut updates = 0;
    let mut clears = 0;
    for change in changes {
        match change {
            ChangeType::Updated(_) => {
                let updates = updates + 1;
            }
            ChangeType::Cleared => clears += 1,
            ChangeType::NoChange => {}
        }
    }
    (updates, clears)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_none_means_leave_alone() {
        let mut current = Some(10.0);
        assert_eq!(apply(&mut current, None), ChangeType::NoChange);
        assert_eq!(current, Some(10.0));
    }

    #[test]
    fn apply_some_none_clears() {
        let mut current = Some(10.0);
        assert_eq!(apply(&mut current, Some(None)), ChangeType::Cleared);
        assert_eq!(current, None);
    }

    #[test]
    fn apply_some_some_sets() {
        let mut current = Some(10.0);
        assert_eq!(
            apply(&mut current, Some(Some(25.0))),
            ChangeType::Updated(25.0)
        );
        assert_eq!(current, Some(25.0));
    }

    #[test]
    fn latest_value_tracks_the_most_recent_update() {
        let changes = vec![
            ChangeType::Updated(1.0),
            ChangeType::NoChange,
            ChangeType::Updated(2.0),
            ChangeType::Cleared,
            ChangeType::Updated(3.0),
        ];
        assert_eq!(latest_value(&changes), Some(3.0));
    }

    #[test]
    fn latest_value_after_clear_is_none() {
        let changes = vec![ChangeType::Updated(1.0), ChangeType::Cleared];
        assert_eq!(latest_value(&changes), None);
    }

    #[test]
    fn describe_changes_counts_updates_and_clears() {
        let changes = vec![
            ChangeType::Updated(1.0),
            ChangeType::Cleared,
            ChangeType::Updated(2.0),
            ChangeType::NoChange,
        ];
        assert_eq!(describe_changes(&changes), (2, 1));
    }
}
