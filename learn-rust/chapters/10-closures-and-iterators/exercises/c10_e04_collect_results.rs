// c10_e04_collect_results.rs
//
// CONCEPT: collecting iterators of `Result` — fail-fast with
//          `collect::<Result<Vec<_>, _>>()` versus gathering *all* errors.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Collecting Results")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::update_entries`
//          returns `Result<(), Vec<(i32, UpdateError)>>`: it applies every
//          update it can and reports *each* failure, not just the first.
// EXPECT:  tests fail: `todo!()` panics.
//
// `Result<Vec<T>, E>` implements `FromIterator<Result<T, E>>`: collecting
// stops at the first `Err` and returns it. That is "fail fast".
// Sometimes you want the opposite — process everything and report every
// problem. `Iterator::partition` or a fold helps there.
//
// YOUR TASK
//   Implement the three functions using iterators. Do not edit the tests.

// I AM NOT DONE

use std::num::ParseIntError;

#[derive(Debug, PartialEq)]
enum UpdateError {
    OutOfBoundsMinMax,
    NotFound,
}

// Type aliases give long types a readable name (clippy's `type_complexity`
// lint nudges you to do this).
type Update = (i32, i64);
type UpdateFailure = (i32, UpdateError);

/// Parses every string; the first failure aborts and is returned.
fn parse_all(raw: &[&str]) -> Result<Vec<i32>, ParseIntError> {
    todo!()
}

/// Checks one update: id must be in `known_ids`, value in `min..=max`.
fn check(update: Update, known_ids: &[i32], min: i64, max: i64) -> Result<Update, UpdateFailure> {
    let (id, value) = update;
    if !known_ids.contains(&id) {
        Err((id, UpdateError::NotFound))
    } else if value < min || value > max {
        Err((id, UpdateError::OutOfBoundsMinMax))
    } else {
        Ok(update)
    }
}

/// Like `update_entries`: returns the accepted updates, or — if *any*
/// update failed — every error (in input order).
fn apply_all(
    updates: &[Update],
    known_ids: &[i32],
    min: i64,
    max: i64,
) -> Result<Vec<Update>, Vec<UpdateFailure>> {
    todo!()
}

/// Sums the values of all updates, but returns None if any id is unknown.
/// Hint: `Option<T>` can be collected/summed just like `Result`.
fn checked_total(updates: &[Update], known_ids: &[i32]) -> Option<i64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_ok() {
        assert_eq!(parse_all(&["1", "2", "3"]), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn parse_all_fails_fast() {
        assert!(parse_all(&["1", "oops", "3"]).is_err());
    }

    #[test]
    fn apply_all_accepts_valid_updates() {
        let got = apply_all(&[(1, 10), (2, 20)], &[1, 2], 0, 100);
        assert_eq!(got, Ok(vec![(1, 10), (2, 20)]));
    }

    #[test]
    fn apply_all_reports_every_error() {
        let got = apply_all(&[(1, 10), (9, 5), (2, 500)], &[1, 2], 0, 100);
        assert_eq!(
            got,
            Err(vec![
                (9, UpdateError::NotFound),
                (2, UpdateError::OutOfBoundsMinMax)
            ])
        );
    }

    #[test]
    fn checked_total_works() {
        assert_eq!(checked_total(&[(1, 10), (2, 5)], &[1, 2]), Some(15));
        assert_eq!(checked_total(&[(1, 10), (3, 5)], &[1, 2]), None);
    }
}
