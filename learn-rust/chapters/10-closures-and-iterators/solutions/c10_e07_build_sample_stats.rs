// c10_e07_build_sample_stats.rs
//
// CONCEPT: writing your own iterator pipelines — `windows`, `sort_by` with a
//          closure key, and `fold`/the `entry` API — over a stream of
//          datapoint-shaped samples.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Iterator adaptors and collect")
// DATABROKER: databroker/src/broker.rs:1503 `AuthorizedAccess::map_entries`
//          and `broker.rs:1114` `EntryReadIterator` — the databroker walks
//          its signal table with iterator adaptors, never index loops.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the three `todo!()` bodies below.
//
// `Sample` stands in for one reading of one signal over time, the way the
// databroker's history buffers or a provider's stream would look.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests, the struct, or the signatures.
//
//   1. `moving_average`: for each contiguous run of `window` samples (use
//      `.windows(window)`), the average of their `value`s, in order. If
//      `samples.len() < window`, return an empty `Vec`.
//   2. `top_n_by`: the `n` samples with the largest `key(sample)`, highest
//      first, ties broken by original order. Use `sort_by` (not
//      `sort_unstable_by` — it isn't order-stable on ties) with
//      `partial_cmp` on `f64` (there are no NaNs in the tests, so
//      `.unwrap()` is fine), then keep only the first `n`.
//   3. `group_sum`: the total `value` for each distinct `path`, as a map.
//      Build it with `fold` or the `entry` API — no index loop.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
struct Sample {
    path: String,
    value: f64,
}

/// The average of every contiguous run of `window` samples, in order.
fn moving_average(samples: &[Sample], window: usize) -> Vec<f64> {
    if window == 0 || samples.len() < window {
        return Vec::new();
    }
    samples
        .windows(window)
        .map(|w| w.iter().map(|s| s.value).sum::<f64>() / window as f64)
        .collect()
}

/// The `n` samples with the largest `key(sample)`, highest first.
fn top_n_by<F>(samples: &[Sample], n: usize, key: F) -> Vec<&Sample>
where
    F: Fn(&Sample) -> f64,
{
    let mut ranked: Vec<&Sample> = samples.iter().collect();
    // WHY: sort_by is a *stable* sort, so equal keys keep their original
    // relative order — that's what "ties broken by original order" means.
    ranked.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap());
    ranked.truncate(n);
    ranked
}

/// The total `value` for each distinct `path`.
fn group_sum(samples: &[Sample]) -> HashMap<String, f64> {
    samples.iter().fold(HashMap::new(), |mut acc, sample| {
        *acc.entry(sample.path.clone()).or_insert(0.0) += sample.value;
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(path: &str, value: f64) -> Sample {
        Sample {
            path: path.to_string(),
            value,
        }
    }

    #[test]
    fn moving_average_over_a_window() {
        let samples = vec![s("v", 1.0), s("v", 2.0), s("v", 3.0), s("v", 4.0)];
        assert_eq!(moving_average(&samples, 2), vec![1.5, 2.5, 3.5]);
    }

    #[test]
    fn moving_average_empty_when_window_too_big() {
        let samples = vec![s("v", 1.0), s("v", 2.0)];
        assert_eq!(moving_average(&samples, 5), Vec::<f64>::new());
    }

    #[test]
    fn top_n_by_orders_descending_and_truncates() {
        let samples = vec![s("a", 3.0), s("b", 9.0), s("c", 1.0), s("d", 9.0)];
        let top = top_n_by(&samples, 2, |sample| sample.value);
        let paths: Vec<&str> = top.iter().map(|s| s.path.as_str()).collect();
        // "b" and "d" tie at 9.0; "b" came first, so it stays first.
        assert_eq!(paths, vec!["b", "d"]);
    }

    #[test]
    fn group_sum_totals_by_path() {
        let samples = vec![
            s("Vehicle.Speed", 10.0),
            s("Vehicle.Speed", 5.0),
            s("Vehicle.IsMoving", 1.0),
        ];
        let sums = group_sum(&samples);
        assert_eq!(sums.get("Vehicle.Speed"), Some(&15.0));
        assert_eq!(sums.get("Vehicle.IsMoving"), Some(&1.0));
        assert_eq!(sums.len(), 2);
    }
}
