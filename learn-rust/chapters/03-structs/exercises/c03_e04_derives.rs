// c03_e04_derives.rs
//
// CONCEPT: `#[derive(...)]` — Debug, Clone, Copy, PartialEq, Eq, Hash.
// GUIDE:   chapters/03-structs/guide.md  (section "Deriving common traits")
// DATABROKER: databroker/src/broker.rs — `#[derive(Debug, PartialEq, Eq,
//             Hash, Clone)] pub enum Field` (Hash + Eq so it can live in a
//             `HashSet<Field>`); databroker/src/types.rs —
//             `#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)] pub struct SignalId`.
// EXPECT:  compile errors E0369 (binary operation `==` cannot be applied),
//          E0277 (`SignalKey` doesn't implement `Debug` / `Hash` ...),
//          E0599 (no method named `clone`).
//
// Rust doesn't give types any abilities for free — not even printing or
// comparing. `derive` asks the compiler to write the obvious implementation:
//   Debug      -> `{:?}` formatting (needed by `assert_eq!` to print failures)
//   Clone      -> `.clone()` (explicit copy)
//   Copy       -> implicit bit-copy on assignment (only for small, heap-free types)
//   PartialEq  -> `==` / `!=`;   Eq -> "== is a true equivalence" (no NaN!)
//   Hash       -> can be a HashMap key / HashSet member (needs Eq too)
//
// YOUR TASK
//   Add the right `#[derive(...)]` lines so the tests compile and pass.
//   - `SignalKey` must support everything the tests do, including being
//     `Copy` and a `HashSet` member.
//   - `Reading` contains an `f64` and a `String`. Figure out which derives
//     are possible (hint: f64 is not `Eq`/`Hash`, String is not `Copy`).
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashSet;

struct SignalKey {
    id: i32,
}

struct Reading {
    key: SignalKey,
    value: f64,
    unit: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_copy_and_comparable() {
        let a = SignalKey { id: 1 };
        let b = a; // Copy: `a` is still usable.
        assert_eq!(a, b);
        assert_ne!(a, SignalKey { id: 2 });
    }

    #[test]
    fn keys_in_a_set() {
        let mut set = HashSet::new();
        set.insert(SignalKey { id: 1 });
        set.insert(SignalKey { id: 1 });
        set.insert(SignalKey { id: 2 });
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn readings_clone_and_compare() {
        let r = Reading {
            key: SignalKey { id: 3 },
            value: 21.5,
            unit: String::from("celsius"),
        };
        let copy = r.clone();
        assert_eq!(r, copy);
        assert_eq!(format!("{:?}", r.key), "SignalKey { id: 3 }");
    }
}
