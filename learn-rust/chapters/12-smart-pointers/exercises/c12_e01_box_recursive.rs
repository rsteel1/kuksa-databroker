// c12_e01_box_recursive.rs
//
// CONCEPT: `Box<T>` — owning a value on the heap so a type can contain
//          itself (recursive types need a known size).
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Box: one owner, on the heap")
// DATABROKER: databroker/src/broker.rs — `provide_actuation` and
//          `register_signals` receive providers as `Box<dyn ... >`
//          (heap-allocated, one owner). doc/wildcard_matching.md describes the
//          path filters modelled here.
// EXPECT:  compile error E0072 (recursive type has infinite size).
//
// A value's size must be known at compile time. `Filter::Not(Filter)` would
// contain a whole Filter, which contains a whole Filter, which… infinite.
// A `Box<Filter>` is just a pointer (8 bytes on 64-bit), so the size becomes
// finite; the inner Filter lives on the heap, owned by the Box.
//
// YOUR TASK
//   1. Fix the enum with `Box` so it compiles (then fix the helper
//      constructors to match).
//   2. Implement `matches` and `depth` (both recursive).
//   Do not edit the tests.

// I AM NOT DONE

/// A tiny filter language over VSS paths.
#[derive(Debug)]
enum Filter {
    /// Matches every path.
    Everything,
    /// Matches paths that start with the prefix.
    Prefix(String),
    /// Matches when both sides match.
    And(Filter, Filter),
    /// Matches when the inner filter does NOT match.
    Not(Filter),
}

fn prefix(p: &str) -> Filter {
    Filter::Prefix(p.to_string())
}

fn and(a: Filter, b: Filter) -> Filter {
    Filter::And(a, b)
}

fn not(f: Filter) -> Filter {
    Filter::Not(f)
}

impl Filter {
    fn matches(&self, path: &str) -> bool {
        todo!()
    }

    /// Everything/Prefix have depth 1; And/Not are 1 + the deepest child.
    fn depth(&self) -> usize {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_filters() {
        assert!(Filter::Everything.matches("Anything.At.All"));
        assert!(prefix("Vehicle.Cabin").matches("Vehicle.Cabin.Temp"));
        assert!(!prefix("Vehicle.Cabin").matches("Vehicle.Speed"));
    }

    #[test]
    fn nested_filters() {
        // Everything under Vehicle except the Cabin.
        let f = and(prefix("Vehicle"), not(prefix("Vehicle.Cabin")));
        assert!(f.matches("Vehicle.Speed"));
        assert!(!f.matches("Vehicle.Cabin.Temp"));
        assert!(!f.matches("Private.X"));
        assert_eq!(f.depth(), 3);
        assert_eq!(not(not(Filter::Everything)).depth(), 3);
    }
}
