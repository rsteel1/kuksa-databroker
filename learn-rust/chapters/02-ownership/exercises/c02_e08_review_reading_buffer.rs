// c02_e08_review_reading_buffer.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 02 (a clone that quietly
//          throws away a mutation, a value read too early, an
//          off-by-one slice bound).
// GUIDE:   chapters/02-ownership/guide.md  (section "Clone: paying for a second copy")
// DATABROKER: databroker/src/broker.rs — `Entry::apply(&mut self, ...)`
//             mutates in place; cloning before the mutation and updating
//             the clone (bug 1 here) would silently drop every update.
// EXPECT:  tests fail: one update doesn't stick, one count is stale, one
//          average is wrong.
//
// A tiny path -> latest-reading store, backed by `Vec<(String, i32)>`
// (no `HashMap` yet — that's chapter 5).
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

fn find_index(entries: &[(String, i32)], path: &str) -> Option<usize> {
    for (i, entry) in entries.iter().enumerate() {
        if entry.0 == path {
            return Some(i);
        }
    }
    None
}

/// Sets `path`'s reading to `value`, appending a new entry if it wasn't
/// there yet. Must mutate `entries` in place.
fn set_reading(entries: &mut Vec<(String, i32)>, path: &str, value: i32) {
    let mut working = entries.clone();
    match find_index(&working, path) {
        Some(i) => working[i].1 = value,
        None => working.push((path.to_string(), value)),
    }
}

/// Applies a batch of readings, then reports how many entries are
/// present in total afterwards.
fn append_batch(entries: &mut Vec<(String, i32)>, batch: &[(String, i32)]) -> usize {
    let count_before = entries.len();
    for (path, value) in batch {
        set_reading(entries, path, *value);
    }
    count_before
}

/// The average of the last `n` entries in insertion order (`0.0` if
/// `entries` is empty).
fn average_of_last(entries: &[(String, i32)], n: usize) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    let take_n = n.min(entries.len());
    let window = &entries[entries.len() - take_n..entries.len() - 1];
    let sum: i64 = window.iter().map(|(_, value)| i64::from(*value)).sum();
    sum as f64 / window.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_reading_updates_in_place() {
        let mut entries = vec![("Vehicle.Speed".to_string(), 10)];
        set_reading(&mut entries, "Vehicle.Speed", 42);
        assert_eq!(entries, vec![("Vehicle.Speed".to_string(), 42)]);
    }

    #[test]
    fn set_reading_appends_new_path() {
        let mut entries = vec![("Vehicle.Speed".to_string(), 10)];
        set_reading(&mut entries, "Vehicle.RPM", 3000);
        assert_eq!(
            entries,
            vec![
                ("Vehicle.Speed".to_string(), 10),
                ("Vehicle.RPM".to_string(), 3000)
            ]
        );
    }

    #[test]
    fn append_batch_reports_total_after() {
        let mut entries = vec![("Vehicle.Speed".to_string(), 10)];
        let reported = append_batch(
            &mut entries,
            &[
                ("Vehicle.RPM".to_string(), 3000),
                ("Vehicle.Gear".to_string(), 3),
            ],
        );
        assert_eq!(reported, 3);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn average_uses_every_requested_entry() {
        let entries = vec![
            ("A".to_string(), 10),
            ("B".to_string(), 20),
            ("C".to_string(), 30),
        ];
        assert_eq!(average_of_last(&entries, 2), 25.0);
        assert_eq!(average_of_last(&entries, 10), 20.0);
    }
}
