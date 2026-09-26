// c17_e06_retry_backoff_circuit_breaker.rs
//
// CONCEPT: recovery strategies for transient failures:
//          * retry only errors that are transient, with capped exponential
//            backoff and a maximum number of attempts (never forever);
//          * a circuit breaker that stops hammering a dependency that keeps
//            failing, then lets one trial call through after a cooldown.
//          Time is *injected* (a `Sleeper` trait, and `now` passed in), so
//          the logic is deterministic to test and works with any runtime.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Retry, backoff and circuit breakers")
// DATABROKER: databroker/src/broker.rs:39 `PROVIDER_UPDATE_FILTER_TIMEOUT` and
//             grpc/kuksa_val_v2/val.rs:158 (one deadline per provider call)
//             bound *how long* a provider may take; the databroker doesn't
//             retry providers, it falls back to the database instead. A
//             provider *client* talking to the databroker is where retry +
//             backoff belongs (reconnecting after the broker restarts).
// EXPECT:  tests fail: `todo!()` panics
//
// YOUR TASK
//   Implement `Backoff::delay_for`, `retry`, and `CircuitBreaker::call`.
//   Do not edit the tests.

// I AM NOT DONE

use std::time::Duration;

/// Errors say whether retrying could help.
trait Transient {
    fn is_transient(&self) -> bool;
}

/// Injected "sleep" so tests don't really wait (and async code can plug in
/// `tokio::time::sleep` later).
trait Sleeper {
    fn sleep(&mut self, duration: Duration);
}

#[derive(Debug, Clone)]
struct Backoff {
    /// Delay before the 2nd attempt.
    base: Duration,
    /// Each further delay is multiplied by this.
    factor: u32,
    /// No single delay is ever longer than this.
    max_delay: Duration,
    /// Total attempts including the first one (>= 1).
    max_attempts: u32,
}

impl Backoff {
    /// Delay to wait *after* failed attempt number `attempt` (1-based):
    /// base * factor^(attempt-1), capped at max_delay.
    /// Must not overflow or panic for large `attempt` (use `checked_pow`,
    /// `checked_mul`/`saturating_mul`, and `min`).
    fn delay_for(&self, attempt: u32) -> Duration {
        todo!()
    }
}

#[derive(Debug, PartialEq)]
enum RetryError<E> {
    /// A non-transient error: returned immediately, without retrying.
    Permanent(E),
    /// Every attempt failed with a transient error; `last` is the final one.
    Exhausted { attempts: u32, last: E },
}

/// Calls `op(attempt)` (attempt is 1-based) until it succeeds, a permanent
/// error occurs, or `policy.max_attempts` attempts have failed. Between
/// attempts, sleeps `policy.delay_for(attempt)`. Never sleeps after the
/// last attempt.
fn retry<T, E: Transient>(
    policy: &Backoff,
    sleeper: &mut impl Sleeper,
    mut op: impl FnMut(u32) -> Result<T, E>,
) -> Result<T, RetryError<E>> {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakerState {
    /// Calls flow; counts consecutive failures.
    Closed { failures: u32 },
    /// Calls are rejected without running until `until`.
    Open { until: Duration },
    /// Cooldown over: the next call is a trial.
    HalfOpen,
}

#[derive(Debug, PartialEq)]
enum BreakerError<E> {
    /// Rejected without calling the operation.
    Open,
    /// The operation ran and failed.
    Inner(E),
}

struct CircuitBreaker {
    failure_threshold: u32,
    cooldown: Duration,
    state: BreakerState,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        CircuitBreaker {
            failure_threshold,
            cooldown,
            state: BreakerState::Closed { failures: 0 },
        }
    }

    /// `now` is "time since some fixed start", supplied by the caller.
    ///
    /// * Open and `now < until`: return Err(Open) *without* calling `op`.
    /// * Open and `now >= until`: become HalfOpen and treat this call as the trial.
    /// * Closed or HalfOpen: run `op`.
    ///   - success: state = Closed { failures: 0 }
    ///   - failure in HalfOpen: Open { until: now + cooldown }
    ///   - failure in Closed: failures += 1; if failures >= threshold,
    ///     Open { until: now + cooldown }
    fn call<T, E>(
        &mut self,
        now: Duration,
        op: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, BreakerError<E>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum NetError {
        Timeout,
        Refused,
        BadRequest,
    }

    impl Transient for NetError {
        fn is_transient(&self) -> bool {
            matches!(self, NetError::Timeout | NetError::Refused)
        }
    }

    #[derive(Default)]
    struct RecordingSleeper(Vec<Duration>);

    impl Sleeper for RecordingSleeper {
        fn sleep(&mut self, duration: Duration) {
            self.0.push(duration);
        }
    }

    fn policy() -> Backoff {
        Backoff {
            base: Duration::from_millis(100),
            factor: 2,
            max_delay: Duration::from_millis(500),
            max_attempts: 5,
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn backoff_grows_and_caps() {
        let p = policy();
        let delays: Vec<Duration> = (1..=5).map(|a| p.delay_for(a)).collect();
        assert_eq!(delays, vec![ms(100), ms(200), ms(400), ms(500), ms(500)]);
        // No overflow panics for silly attempt numbers.
        assert_eq!(p.delay_for(1_000), ms(500));
        assert_eq!(p.delay_for(u32::MAX), ms(500));
    }

    #[test]
    fn retries_transient_until_success() {
        let mut sleeper = RecordingSleeper::default();
        let result = retry(&policy(), &mut sleeper, |attempt| {
            if attempt < 3 {
                Err(NetError::Timeout)
            } else {
                Ok(attempt)
            }
        });
        assert_eq!(result, Ok(3));
        assert_eq!(sleeper.0, vec![ms(100), ms(200)]);
    }

    #[test]
    fn permanent_errors_are_not_retried() {
        let mut sleeper = RecordingSleeper::default();
        let mut calls = 0;
        let result: Result<(), _> = retry(&policy(), &mut sleeper, |_| {
            calls += 1;
            Err(NetError::BadRequest)
        });
        assert_eq!(result, Err(RetryError::Permanent(NetError::BadRequest)));
        assert_eq!(calls, 1);
        assert!(sleeper.0.is_empty());
    }

    #[test]
    fn gives_up_after_max_attempts_without_a_final_sleep() {
        let mut sleeper = RecordingSleeper::default();
        let result: Result<(), _> = retry(&policy(), &mut sleeper, |_| Err(NetError::Refused));
        assert_eq!(
            result,
            Err(RetryError::Exhausted {
                attempts: 5,
                last: NetError::Refused
            })
        );
        assert_eq!(sleeper.0, vec![ms(100), ms(200), ms(400), ms(500)]);
    }

    #[test]
    fn breaker_opens_after_threshold_and_rejects_fast() {
        let mut b = CircuitBreaker::new(3, ms(1000));
        for t in 0..3 {
            let r: Result<(), _> = b.call(ms(t), || Err(NetError::Timeout));
            assert_eq!(r, Err(BreakerError::Inner(NetError::Timeout)));
        }
        assert_eq!(b.state, BreakerState::Open { until: ms(1002) });
        let mut ran = false;
        let r: Result<(), BreakerError<NetError>> = b.call(ms(500), || {
            ran = true;
            Ok(())
        });
        assert_eq!(r, Err(BreakerError::Open));
        assert!(!ran, "an open breaker must not call the operation");
    }

    #[test]
    fn success_resets_the_failure_count() {
        let mut b = CircuitBreaker::new(2, ms(1000));
        let _: Result<(), _> = b.call(ms(0), || Err(NetError::Timeout));
        b.call(ms(1), || Ok::<_, NetError>(())).unwrap();
        assert_eq!(b.state, BreakerState::Closed { failures: 0 });
        let _: Result<(), _> = b.call(ms(2), || Err(NetError::Timeout));
        assert_eq!(b.state, BreakerState::Closed { failures: 1 });
    }

    #[test]
    fn half_open_trial() {
        let mut b = CircuitBreaker::new(1, ms(100));
        let _: Result<(), _> = b.call(ms(0), || Err(NetError::Timeout));
        assert_eq!(b.state, BreakerState::Open { until: ms(100) });

        // Trial fails: open again for another cooldown.
        let _: Result<(), _> = b.call(ms(150), || Err(NetError::Timeout));
        assert_eq!(b.state, BreakerState::Open { until: ms(250) });

        // Trial succeeds: closed.
        assert_eq!(b.call(ms(250), || Ok::<_, NetError>(9)), Ok(9));
        assert_eq!(b.state, BreakerState::Closed { failures: 0 });
    }
}
