// c14_e09_build_typed_cell.rs
//
// CONCEPT: `dyn Any` plus `downcast_ref`, restricted to a closed set of
//          types with the sealed-trait pattern — a type-erased cell that
//          can hold any *allowed* signal value, and a helper that finds the
//          first value of a given concrete type in a heterogeneous list.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Any and downcasting")
// DATABROKER: `databroker-proto` uses `std::any::Any` for its generated
//          message types; `DataValue` in `databroker/src/types.rs` is the
//          closed, sealed-style enum this exercise's `SignalValue` mimics
//          with types instead of variants.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the four `todo!()` bodies below.
//
// `SignalValue` is sealed: only `i64`, `bool` and `String` implement it, and
// nothing outside this file can add a new impl (the `sealed` module isn't
// `pub`). That's what lets `TypedCell` promise "any signal value" without
// promising "any type at all".
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests, the traits, or the signatures.
//
//   1. `TypedCell::new`: box `value` as `dyn Any` and store it.
//   2. `TypedCell::get`: `Some(&T)` if the stored value's concrete type is
//      `T`, else `None`. Don't panic on a mismatch.
//   3. `TypedCell::set`: replace the stored value with a freshly boxed
//      `value` (the new value doesn't have to be the same type as the old
//      one — that's the point of storing `dyn Any`).
//   4. `first_of_type`: the first element of `values` whose concrete type
//      is `T`, or `None` if none match.

use std::any::Any;

mod sealed {
    pub trait Sealed {}
    impl Sealed for i64 {}
    impl Sealed for bool {}
    impl Sealed for String {}
}

/// The closed set of types a `TypedCell` may hold.
trait SignalValue: sealed::Sealed + 'static {}
impl SignalValue for i64 {}
impl SignalValue for bool {}
impl SignalValue for String {}

struct TypedCell {
    value: Box<dyn Any>,
}

impl TypedCell {
    fn new<T: SignalValue>(value: T) -> Self {
        TypedCell {
            value: Box::new(value),
        }
    }

    /// `Some(&T)` if the stored value's concrete type is `T`.
    fn get<T: SignalValue>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }

    /// Replaces the stored value, possibly with a different `SignalValue` type.
    fn set<T: SignalValue>(&mut self, value: T) {
        self.value = Box::new(value);
    }
}

/// The first element of `values` whose concrete type is `T`.
fn first_of_type<T: 'static>(values: &[Box<dyn Any>]) -> Option<&T> {
    values.iter().find_map(|v| v.downcast_ref::<T>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_matches_the_stored_concrete_type() {
        let cell = TypedCell::new(42i64);
        assert_eq!(cell.get::<i64>(), Some(&42));
        assert_eq!(cell.get::<bool>(), None);
    }

    #[test]
    fn set_can_change_the_stored_type() {
        let mut cell = TypedCell::new(42i64);
        cell.set("hello".to_string());
        assert_eq!(cell.get::<i64>(), None);
        assert_eq!(cell.get::<String>(), Some(&"hello".to_string()));
    }

    #[test]
    fn set_can_replace_with_the_same_type() {
        let mut cell = TypedCell::new(true);
        cell.set(false);
        assert_eq!(cell.get::<bool>(), Some(&false));
    }

    #[test]
    fn first_of_type_finds_the_right_element() {
        let values: Vec<Box<dyn Any>> =
            vec![Box::new(1i64), Box::new("hi".to_string()), Box::new(true)];
        assert_eq!(first_of_type::<String>(&values), Some(&"hi".to_string()));
        assert_eq!(first_of_type::<bool>(&values), Some(&true));
        assert_eq!(first_of_type::<f64>(&values), None);
    }
}
