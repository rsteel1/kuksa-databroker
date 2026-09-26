// c32_e01_unit_tests_and_should_panic.rs
//
// CONCEPT: writing tests yourself — assert macros, `#[should_panic]`, `matches!`, tests returning Result.
// GUIDE:   chapters/32-testing/guide.md  (section "Unit tests: the toolbox")
// DATABROKER: databroker/src/broker.rs — `subscribe` (buffer size rules) and its tests
//             `test_subscribe_buffersize_out_of_range`; types.rs `#[test] fn test_string_greater_than`
// EXPECT:  tests fail: the test bodies are `todo!()`
//
// THIS CHAPTER IS DIFFERENT: the code under test is correct and you must NOT
// change it. Instead, YOU write the bodies of the tests. Replace each
// `todo!()` with real assertions, following the comment above each test.
// (Yes, deleting the `todo!()` would make a test "pass" — the point is to
// practise writing checks that would catch a real bug. Try this: after you
// finish, break the code under test on purpose and watch your tests fail.)

// ---------------- code under test (do not change) ----------------

const MAX_SUBSCRIBE_BUFFER_SIZE: usize = 1000;

#[derive(Debug, PartialEq)]
enum SubscriptionError {
    InvalidBufferSize(usize),
}

/// Mirrors `AuthorizedAccess::subscribe`: no buffer -> capacity 1 (latest
/// value only); otherwise the requested history plus 1 for the latest value.
fn channel_capacity(buffer_size: Option<usize>) -> Result<usize, SubscriptionError> {
    match buffer_size {
        None => Ok(1),
        Some(cap) if cap > MAX_SUBSCRIBE_BUFFER_SIZE => {
            Err(SubscriptionError::InvalidBufferSize(cap))
        }
        Some(cap) => Ok(cap + 1),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SignalId(i32);

impl SignalId {
    /// Panics on negative ids: they can only come from a programming error.
    fn new(id: i32) -> Self {
        assert!(id >= 0, "signal ids are never negative, got {id}");
        SignalId(id)
    }
}

// ------------------------------ tests ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // `None` gives capacity 1, `Some(0)` gives 1, `Some(10)` gives 11.
    // Use assert_eq!(actual, expected).
    #[test]
    fn capacities() {
        assert_eq!(channel_capacity(None), Ok(1));
        assert_eq!(channel_capacity(Some(0)), Ok(1));
        assert_eq!(channel_capacity(Some(10)), Ok(11));
    }

    // The boundary: 1000 is allowed (capacity 1001), 1001 is rejected.
    // Test BOTH sides of the boundary — off-by-one bugs live exactly there.
    // For the error use `matches!` inside `assert!`, and check the carried value.
    #[test]
    fn boundary() {
        assert_eq!(channel_capacity(Some(1000)), Ok(1001));
        assert!(matches!(
            channel_capacity(Some(1001)),
            Err(SubscriptionError::InvalidBufferSize(1001))
        ));
    }

    // A test may return Result: `?` then fails the test on Err.
    // Get the capacity for Some(99) with `?` and assert it is 100.
    #[test]
    fn with_question_mark() -> Result<(), SubscriptionError> {
        let capacity = channel_capacity(Some(99))?;
        assert_eq!(capacity, 100);
        Ok(())
    }

    // Make this test pass only if SignalId::new panics with the right message.
    // Fill in `expected = "..."` with a distinctive part of the panic message,
    // then call the constructor with a negative id.
    #[should_panic(expected = "signal ids are never negative")]
    #[test]
    fn negative_id_panics() {
        SignalId::new(-1);
    }

    // Non-negative ids are accepted. Add a custom failure message to the
    // assertion (the extra format arguments after the two values).
    #[test]
    fn valid_ids() {
        for id in [0, 1, i32::MAX] {
            assert_eq!(
                SignalId::new(id),
                SignalId(id),
                "id {id} should be accepted"
            );
        }
    }
}
