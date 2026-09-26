// c23_e01_async_fn_in_trait.rs
//
// CONCEPT: native `async fn` in traits (Rust 1.75+) with generics — static
//          dispatch, no macro, no allocation.
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "async fn in traits")
// DATABROKER: databroker/src/broker.rs — `trait SignalProvider` has
//             `async fn update_filter` and `async fn get_signals_values_from_provider`
//             (there written with `#[async_trait]`, see the next exercise for why)
// EXPECT:  tests fail: `todo!()` panics
//
// When the caller knows the concrete type (via a generic `S: SignalSource`),
// the compiler generates a specialised copy of `read_all` per type
// ("monomorphization") and the async trait method is just an ordinary
// function returning an anonymous future. That's zero-cost.
//
// YOUR TASK
//   Implement the trait for `Constant` and `Table`, and write `read_all`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::time::Duration;

trait SignalSource {
    fn name(&self) -> &str;
    async fn read(&self, id: i32) -> Option<f64>;
}

/// Answers every id with the same value, instantly.
struct Constant {
    value: f64,
}

/// Looks ids up in a table after a simulated network `latency`.
struct Table {
    values: HashMap<i32, f64>,
    latency: Duration,
}

impl SignalSource for Constant {
    fn name(&self) -> &str {
        "constant"
    }

    async fn read(&self, _id: i32) -> Option<f64> {
        todo!()
    }
}

impl SignalSource for Table {
    fn name(&self) -> &str {
        "table"
    }

    async fn read(&self, id: i32) -> Option<f64> {
        todo!()
    }
}

/// Read every id from `source`, in order (one after another is fine here).
async fn read_all<S: SignalSource>(source: &S, ids: &[i32]) -> Vec<Option<f64>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn constant_source() {
        let source = Constant { value: 1.5 };
        assert_eq!(source.name(), "constant");
        assert_eq!(read_all(&source, &[1, 2]).await, vec![Some(1.5), Some(1.5)]);
    }

    #[tokio::test(start_paused = true)]
    async fn table_source() {
        let source = Table {
            values: HashMap::from([(1, 10.0), (3, 30.0)]),
            latency: Duration::from_millis(5),
        };
        let start = tokio::time::Instant::now();
        assert_eq!(
            read_all(&source, &[1, 2, 3]).await,
            vec![Some(10.0), None, Some(30.0)]
        );
        assert_eq!(start.elapsed(), Duration::from_millis(15));
    }
}
