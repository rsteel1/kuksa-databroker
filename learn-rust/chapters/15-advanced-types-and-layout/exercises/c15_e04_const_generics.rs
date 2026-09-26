// c15_e04_const_generics.rs
//
// CONCEPT: const generics: types and functions parameterised by a value (`const N: usize`)
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Const generics")
// DATABROKER: databroker/src/broker.rs — `subscribe` keeps at most `buffer_size + 1`
//             updates for a slow subscriber (`broadcast::channel(capacity)`). Here
//             you build a fixed-capacity buffer whose capacity is part of its type.
// EXPECT:  tests fail: `todo!()` panics
//
// `[T; N]` has always been generic over `N`. Const generics let *your* types
// and functions do the same: `RingBuffer<T, 4>` and `RingBuffer<T, 8>` are
// different types, the capacity costs no memory at runtime, and it can't
// be accidentally changed.
//
// YOUR TASK
//   Implement the `todo!()`s. The buffer keeps the newest `N` items; pushing
//   into a full buffer evicts (and returns) the oldest item.
//   Do not edit the tests.

// I AM NOT DONE

/// Keeps the newest `N` items in insertion order.
struct RingBuffer<T, const N: usize> {
    slots: [Option<T>; N],
    /// Index of the oldest item.
    head: usize,
    len: usize,
}

impl<T, const N: usize> RingBuffer<T, N> {
    fn new() -> Self {
        todo!()
    }

    /// The capacity is a compile-time constant, available without an instance.
    const CAPACITY: usize = N;

    fn len(&self) -> usize {
        todo!()
    }

    /// Adds `item`. If the buffer was full, returns the evicted oldest item.
    fn push(&mut self, item: T) -> Option<T> {
        todo!()
    }

    /// Iterates from oldest to newest.
    fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        // TODO: yield items from oldest to newest.
        std::iter::empty()
    }
}

/// Averages exactly `N` samples. The array length is checked by the compiler.
fn average<const N: usize>(samples: [f64; N]) -> Option<f64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items<T: Clone, const N: usize>(rb: &RingBuffer<T, N>) -> Vec<T> {
        rb.iter().cloned().collect()
    }

    #[test]
    fn fills_up_in_order() {
        let mut rb: RingBuffer<&str, 3> = RingBuffer::new();
        assert_eq!(rb.push("a"), None);
        assert_eq!(rb.push("b"), None);
        assert_eq!(items(&rb), ["a", "b"]);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn evicts_oldest_when_full() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        for v in 1..=3 {
            rb.push(v);
        }
        assert_eq!(rb.push(4), Some(1));
        assert_eq!(rb.push(5), Some(2));
        assert_eq!(items(&rb), [3, 4, 5]);
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn works_with_non_copy_items() {
        let mut rb: RingBuffer<String, 2> = RingBuffer::new();
        rb.push("Vehicle.Speed".to_string());
        rb.push("Vehicle.Width".to_string());
        assert_eq!(
            rb.push("Vehicle.Height".to_string()).as_deref(),
            Some("Vehicle.Speed")
        );
    }

    #[test]
    fn capacity_is_part_of_the_type() {
        assert_eq!(RingBuffer::<u8, 16>::CAPACITY, 16);
        assert_eq!(
            std::mem::size_of::<RingBuffer<u8, 4>>(),
            std::mem::size_of::<[Option<u8>; 4]>() + 2 * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn zero_capacity_keeps_nothing() {
        let mut rb: RingBuffer<i32, 0> = RingBuffer::new();
        assert_eq!(rb.push(1), Some(1));
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn average_of_fixed_samples() {
        assert_eq!(average([1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(average::<0>([]), None);
    }
}
