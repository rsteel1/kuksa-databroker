# Chapter 32 hints

## c32_e01_unit_tests_and_should_panic
- `assert_eq!(channel_capacity(None), Ok(1));` — `Result` and the error enum
  derive `PartialEq`, so whole results can be compared.
- Boundary: check `Some(1000)` → `Ok(1001)` and use
  `assert!(matches!(channel_capacity(Some(1001)), Err(SubscriptionError::InvalidBufferSize(1001))));`
- Result-returning test: `let capacity = channel_capacity(Some(99))?; assert_eq!(capacity, 100); Ok(())`.
- `#[should_panic(expected = "signal ids are never negative")]` — `expected`
  only needs to be a substring of the panic message. Body: `SignalId::new(-1);`
- Custom message: `assert_eq!(a, b, "id {id} should be accepted");`

## c32_e02_async_tests_paused_time
Keep `let start = Instant::now();` and `let mut last_emitted: Option<Instant> = None;`.
Loop with `while let Some(value) = rx.recv().await { ... }` — it ends when all
senders are dropped. A value is due when
`last_emitted.is_none_or(|last| now.duration_since(last) >= interval)`.
Record `now.duration_since(start).as_millis()`.
Your test: `let rx = produce(vec![(0, 1.0), (100, 2.0), (100, 3.0)]);` then
assert the exact `Vec` of `(ms, value)` pairs.

## c32_e03_test_doubles_for_traits
- Static: `Ok(ids.iter().filter_map(|id| self.values.get(id).map(|v| (*id, *v))).collect())`.
- Failing: `Err(())`.
- Hanging: `std::future::pending().await` — a future that is never ready; the
  type is inferred from the function's return type.
- Counting: `self.calls.fetch_add(1, Ordering::SeqCst); self.inner.values(ids).await`.
- Unavailable: `is_available` returns `false`; `values` is `unreachable!("...")`.
All others are available (`true`).

## c32_e04_test_builders
`signal`: push `SignalSpec { path: path.to_string(), ..Default::default() }` and return `self`.
`last`: `self.signals.last_mut().expect("call .signal(..) before modifying a signal")`
— the test checks that exact message.
Modifiers: `self.last().exists = true; self`.
`build(&self)`: `Scenario { signals: self.signals.clone() }`.
`run`: build `Db` with `.filter(|s| s.exists).map(|s| (s.path.clone(), s.value)).collect()`,
`Perms` similarly from `authorized`, and the path list with
`self.signals.iter().map(|s| s.path.as_str()).collect::<Vec<_>>()`.

## c32_e05_property_testing
Start with the generator. `prop_oneof![a, b, c]` picks one of several
strategies, each mapped into a variant with `.prop_map(Value::Bool)` and
so on. Use `any::<bool>()`, `any::<i64>()`, and
`prop::collection::vec(any::<i64>(), 0..5)` (a range starting at 0 includes
the empty array).

Strings are the key. `any::<String>()` almost never produces a backslash.
A regex strategy such as `"[a-z\\\\\n]{0,10}"` (letters, backslash, newline)
makes them common. Include both.

Now run the tests and read the "minimal failing input". Encode it by hand
with the rules in the doc comment. Which character isn't escaped? Then
think about what `"".split(',')` returns.

## c32_e06_checkpoint_part5
Turn each sentence of the contract into a tiny scenario: a list of calls
and the expected `lowest()` map. Write a helper that builds `F::default()`,
applies the calls, compares, and returns `Err(String)` with a readable
message. `?` then chains scenarios.

Which sentences are easy to get wrong? "REPLACING any interval that
subscription had" (can a subscription become *slower*?) and "recomputed
when a subscription goes away" (what if the removed one was the fastest?).

For a deeper check, write a *model*: a `HashMap<(sub, signal), ms>` that
obviously follows the contract. Generate a few hundred random call
sequences (a tiny linear congruential generator is enough, and it's
deterministic), apply each call to both the model and `F`, and compare
after every step.
