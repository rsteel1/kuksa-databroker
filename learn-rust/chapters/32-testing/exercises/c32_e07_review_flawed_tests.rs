// c32_e07_review_flawed_tests.rs
//
// CONCEPT: code review — but this time the bugs are in the TESTS, not the
//          code under test. All 3 planted bugs are flawed tests that pass
//          for the wrong reason (or for no reason at all), the exact kind
//          of test a reviewer must catch: they give false confidence.
// GUIDE:   chapters/32-testing/guide.md  (section "Flawed tests that pass wrongly")
// DATABROKER: databroker/src/broker.rs:891-905 —
//             `Subscriptions::cleanup_change_subscriptions` removes a
//             subscription when `sender.receiver_count() == 0` OR its
//             permissions expired, and nothing else; its real test asserts
//             on the returned uuids, not just that the call didn't panic.
// EXPECT:  `cargo test` fails: `expired_subscriptions_are_removed` fails
//          with a wrong `assert_eq!` — but that is only ONE of the 3 bugs.
//          The other two currently "pass", which is itself the problem:
//          they would keep passing even if `cleanup` were broken.
//
// THIS CHAPTER IS DIFFERENT (like c32_e01): the code under test below is
// correct and you must NOT change it. All 3 bugs are in `mod tests`.
//
// THIS FILE HAS EXACTLY 3 BUGS, ALL IN THE TESTS.
//
// YOUR TASK
//   Find and fix all 3 flawed tests so each one actually exercises the
//   behaviour its name promises. For each one, add a one-line comment right
//   above your fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`. Do not change anything above `mod tests`.

// I AM NOT DONE

use std::time::{Duration, Instant};

// ---------------- code under test (do not change) ----------------

#[derive(Debug, Clone, PartialEq)]
struct Subscription {
    id: u32,
    receiver_count: usize,
    expires_at: Option<Instant>,
}

impl Subscription {
    /// Panics on id 0: ids are assigned starting at 1, so 0 can only mean a
    /// programming error upstream.
    fn new(id: u32, receiver_count: usize, expires_at: Option<Instant>) -> Self {
        assert!(
            id != 0,
            "subscription ids are assigned starting at 1, got {id}"
        );
        Subscription {
            id,
            receiver_count,
            expires_at,
        }
    }
}

/// Mirrors `Subscriptions::cleanup_change_subscriptions`: removes every
/// subscription with no receivers left, or whose permissions have expired.
/// Returns the ids removed, in their original order.
fn cleanup(subs: &mut Vec<Subscription>, now: Instant) -> Vec<u32> {
    let mut removed = Vec::new();
    subs.retain(|sub| {
        let dead = sub.receiver_count == 0;
        let expired = sub.expires_at.is_some_and(|at| at <= now);
        if dead || expired {
            removed.push(sub.id);
            false
        } else {
            true
        }
    });
    removed
}

// ------------------------------ tests ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn alive(id: u32) -> Subscription {
        Subscription::new(id, 1, None)
    }

    // This test calls `cleanup` but never looks at what it did: it "passes"
    // no matter what `cleanup` returns or does to `subs`, including if
    // `cleanup` removed nothing, removed everything, or panicked-and-was-
    // caught. Check that the dead subscription (0 receivers) was removed
    // and the alive one was kept, on BOTH the returned ids and `subs`
    // itself.
    #[test]
    fn dead_subscriptions_are_removed() {
        let mut subs = vec![Subscription::new(1, 0, None), alive(2)];
        let now = Instant::now();
        cleanup(&mut subs, now);
    }

    // `#[should_panic]` with no `expected = "..."` matches ANY panic, for
    // ANY reason — including a panic from a bug somewhere completely
    // different from the one this test claims to check. Add an `expected`
    // string that is actually part of `Subscription::new`'s panic message.
    #[test]
    #[should_panic]
    fn zero_id_is_rejected() {
        Subscription::new(0, 1, None);
    }

    // The expected value here was written down wrong: only the EXPIRED
    // subscription (id 1) should be removed, not the one that is merely
    // alive-but-unrelated (id 2). Fix the expected value, not `cleanup`.
    #[test]
    fn expired_subscriptions_are_removed() {
        let expired_at = Instant::now() - Duration::from_secs(1);
        let mut subs = vec![Subscription::new(1, 1, Some(expired_at)), alive(2)];
        let now = Instant::now();
        let removed = cleanup(&mut subs, now);
        assert_eq!(removed, vec![1, 2]);
        assert_eq!(subs, vec![alive(2)]);
    }
}
