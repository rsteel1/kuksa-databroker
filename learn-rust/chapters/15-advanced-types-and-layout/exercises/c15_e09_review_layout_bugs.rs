// c15_e09_review_layout_bugs.rs
//
// CONCEPT: code review. Three independent pieces of code, each compiling
//          cleanly, each with one planted bug typical of const-generic and
//          layout-adjacent code: a hardcoded divisor that ignores the
//          const parameter, an off-by-one in a fixed-capacity window's
//          length bookkeeping, and an interning cache that never actually
//          checks the cache before allocating.
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Const generics")
// DATABROKER: chapter 15's `c15_e07_box_str_rc_str` interner is the
//          correct version of the third bug below — hand a fresh
//          `Arc::from(path)` to every caller and you've defeated the whole
//          point of interning (deduplicated, `ptr_eq`-able allocations).
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests or the signatures.
//   For each bug, add a one-line comment directly above your fix:
//   `// BUG 1: <what was wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

use std::collections::HashMap;
use std::rc::Rc;

// --- Bug 1: hardcoded divisor ----------------------------------------------

/// The mean of exactly `N` samples.
fn average<const N: usize>(samples: [f64; N]) -> f64 {
    samples.iter().sum::<f64>() / 10.0
}

// --- Bug 2: off-by-one in a fixed-capacity window --------------------------

/// Keeps the newest `N` values; `len()` never exceeds `N`.
struct Window<const N: usize> {
    data: [i32; N],
    next: usize,
    len: usize,
}

impl<const N: usize> Window<N> {
    fn new() -> Self {
        Window {
            data: [0; N],
            next: 0,
            len: 0,
        }
    }

    fn push(&mut self, value: i32) {
        self.data[self.next] = value;
        self.next = (self.next + 1) % N;
        if self.len <= N {
            self.len += 1;
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

// --- Bug 3: interner that never reuses a cached allocation -----------------

/// Deduplicates repeated strings: interning the same text twice must
/// return handles to the SAME allocation.
#[derive(Default)]
struct Interner {
    cache: HashMap<String, Rc<str>>,
}

impl Interner {
    fn new() -> Self {
        Self::default()
    }

    fn intern(&mut self, text: &str) -> Rc<str> {
        let shared: Rc<str> = Rc::from(text);
        self.cache.insert(text.to_string(), Rc::clone(&shared));
        shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_divides_by_the_actual_count() {
        assert_eq!(average([2.0, 4.0, 6.0]), 4.0);
        assert_eq!(average([1.0, 1.0, 1.0, 1.0, 1.0]), 1.0);
    }

    #[test]
    fn window_len_never_exceeds_capacity() {
        let mut window: Window<3> = Window::new();
        for v in 1..=5 {
            window.push(v);
        }
        assert_eq!(window.len(), 3);
    }

    #[test]
    fn interner_reuses_the_same_allocation() {
        let mut interner = Interner::new();
        let first = interner.intern("Vehicle.Speed");
        let second = interner.intern("Vehicle.Speed");
        assert!(Rc::ptr_eq(&first, &second));
    }
}
