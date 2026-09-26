// c05_e05_indexmap_order.rs
//
// CONCEPT: IndexMap — a hash map that remembers insertion order.
// GUIDE:   chapters/05-collections/guide.md  (section "IndexMap: when order is part of the contract")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::get_values_broker`
//             ("Preserve request order: the gRPC contract promises that data
//             points are returned in the same order as the requested signals")
//             and `GetValuesProviderResponse { entries: IndexMap<SignalId, Datapoint> }`.
// EXPECT:  tests fail: `todo!()` panics
//
// GetValues asks for several signals. Some are served by a provider (which
// may answer only partially), the rest come from the database. The answer
// must list values in exactly the requested order. A HashMap would scramble
// it; an IndexMap iterates in insertion order.
//
// YOUR TASK
//   Implement `get_values`. For each requested id, in order:
//     1. use the provider's value if the provider returned one,
//     2. otherwise fall back to the database value,
//     3. otherwise fail with Err(id) (first missing id wins).
//   Do not edit the tests.

// I AM NOT DONE

use indexmap::IndexMap;
use std::collections::HashMap;

fn get_values(
    requested: &[i32],
    provider: &HashMap<i32, f64>,
    database: &HashMap<i32, f64>,
) -> Result<IndexMap<i32, f64>, i32> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_request_order() {
        let db = HashMap::from([(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)]);
        let result = get_values(&[4, 1, 3, 2], &HashMap::new(), &db).unwrap();
        let ids: Vec<i32> = result.keys().copied().collect();
        assert_eq!(ids, vec![4, 1, 3, 2]);
    }

    #[test]
    fn provider_wins_and_partial_answers_fall_back() {
        let provider = HashMap::from([(2, 200.0)]);
        let db = HashMap::from([(1, 1.0), (2, 2.0), (3, 3.0)]);
        let result = get_values(&[3, 2, 1], &provider, &db).unwrap();
        let pairs: Vec<(i32, f64)> = result.into_iter().collect();
        assert_eq!(pairs, vec![(3, 3.0), (2, 200.0), (1, 1.0)]);
    }

    #[test]
    fn missing_signal_is_an_error() {
        let db = HashMap::from([(1, 1.0)]);
        assert_eq!(get_values(&[1, 9, 8], &HashMap::new(), &db), Err(9));
    }
}
