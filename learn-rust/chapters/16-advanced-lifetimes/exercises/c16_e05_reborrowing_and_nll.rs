// c16_e05_reborrowing_and_nll.rs
//
// CONCEPT: implicit reborrows, why generics can defeat them, and one known limit of NLL
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "Reborrowing, two-phase borrows and NLL")
// DATABROKER: databroker/src/broker.rs — `update_entries` calls `db_write.update(..)` once
//             per entry in a loop. Each method call takes a fresh `&mut db_write`
//             (auto-ref), the method-call cousin of the reborrow you'll write here.
// EXPECT:  compile errors: E0382 (use of moved value: `out`) and
//          E0502 (cannot borrow `*map` as mutable because it is also borrowed as immutable)
//
// `&mut T` is not `Copy`. When you pass `r: &mut T` to a function that takes
// `&mut T`, the compiler silently inserts a *reborrow* (`&mut *r`), so `r` is
// usable again afterwards. But when the parameter is a generic `W` that
// merely happens to be `&mut String`, no reborrow is inserted and the `&mut`
// is *moved*.
//
// Non-lexical lifetimes (NLL) end a borrow at its last use, not at the end of
// the block. NLL still rejects the "return a borrow on one path, mutate on
// another" pattern in `value_or_insert`: a borrow that is *returned* is treated
// as lasting for the rest of the function. The next-generation borrow
// checker (Polonius) accepts it; for now, restructure.
//
// YOUR TASK
//   1. Fix `log_all` with an explicit reborrow. Keep `record` generic.
//   2. Rewrite `value_or_insert` so it compiles. It must not overwrite an
//      existing value.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::fmt::Write;

/// Writes one line to any `fmt::Write` sink.
fn record<W: Write>(mut sink: W, line: &str) {
    writeln!(sink, "{line}").expect("writing to a String cannot fail");
}

/// Logs every line through `record`, reusing the same `&mut String`.
fn log_all(out: &mut String, lines: &[&str]) {
    for line in lines {
        // `out` itself is not moved into the generic parameter.
        record(out, line);
    }
}

/// Returns the value for `key`, inserting 0 first if it is missing.
fn value_or_insert<'m>(map: &'m mut HashMap<String, i64>, key: &str) -> &'m i64 {
    if let Some(v) = map.get(key) {
        return v;
    }
    map.insert(key.to_string(), 0);
    map.get(key).expect("just inserted")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_every_line() {
        let mut out = String::new();
        log_all(&mut out, &["Vehicle.Speed=10", "Vehicle.Speed=20"]);
        log_all(&mut out, &[]);
        assert_eq!(out, "Vehicle.Speed=10\nVehicle.Speed=20\n");
    }

    #[test]
    fn inserts_only_when_missing() {
        let mut map = HashMap::from([("Vehicle.Speed".to_string(), 42)]);
        assert_eq!(*value_or_insert(&mut map, "Vehicle.Speed"), 42);
        assert_eq!(*value_or_insert(&mut map, "Vehicle.Width"), 0);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn things_nll_and_two_phase_borrows_allow() {
        // Two-phase borrow: `v.len()` is evaluated before `push` activates its `&mut`.
        let mut v = vec![1usize];
        v.push(v.len());
        assert_eq!(v, [1, 1]);

        // NLL: `first`'s borrow ends at its last use, so the push afterwards is fine.
        let first = &v[0];
        assert_eq!(*first, 1);
        v.push(3);
        assert_eq!(v.len(), 3);

        // Implicit reborrow: a concrete `&mut Vec` parameter doesn't consume `r`.
        fn push_one(v: &mut Vec<usize>) {
            v.push(1);
        }
        let r = &mut v;
        push_one(r);
        push_one(r);
        assert_eq!(v.len(), 5);
    }
}
