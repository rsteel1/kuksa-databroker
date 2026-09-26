// c09_e05_generic_bounds_and_where.rs
//
// CONCEPT: generic functions/structs and trait bounds (`T: PartialOrd + Copy`,
//          `where` clauses); monomorphization.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Generics and trait bounds")
// DATABROKER: databroker/src/broker.rs — `Entry::validate_value_min_max`
//             checks a value against optional min/max; types.rs implements
//             `greater_than`/`less_than` for every numeric DataValue variant
//             by hand (one match arm per type pair). Generics are how you'd
//             write such logic once for all numeric types.
// EXPECT:  compile error E0369 (binary operation `>` cannot be applied to type `T`)
//
// A generic function works for many types, but inside it you may only use
// what the *bounds* promise. `T` alone promises nothing — not even `>` or
// copying. Add the traits you need: `PartialOrd` gives `<`/`>`, `Copy` lets
// you return a `T` out of a slice by value.
//
// The compiler generates a specialised copy of the function for each concrete
// type you use (monomorphization): zero runtime cost, like hand-written code.
//
// YOUR TASK
//   Add the missing bounds (use a `where` clause on at least one function),
//   and implement `Range::contains` and `clamp_all`. Do not edit the tests.

// I AM NOT DONE

/// Largest element, or None for an empty slice.
fn max_of<T>(values: &[T]) -> Option<T> {
    let mut iter = values.iter();
    let mut best = *iter.next()?;
    for v in iter {
        if *v > best {
            best = *v;
        }
    }
    Some(best)
}

/// Inclusive range with optional ends, like VSS min/max metadata.
struct Range<T> {
    min: Option<T>,
    max: Option<T>,
}

impl<T> Range<T> {
    /// True if `value` is within [min, max]; a missing end is unbounded.
    fn contains(&self, value: &T) -> bool {
        todo!()
    }
}

/// Clamps every element into [min, max] in place.
fn clamp_all<T>(values: &mut [T], min: T, max: T) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_of_many_types() {
        assert_eq!(max_of(&[3, 9, 2]), Some(9));
        assert_eq!(max_of(&[1.5f32, -2.0]), Some(1.5));
        assert_eq!(max_of(&['a', 'z', 'm']), Some('z'));
        assert_eq!(max_of::<u8>(&[]), None);
    }

    #[test]
    fn range_contains() {
        let speed = Range {
            min: Some(0),
            max: Some(250),
        };
        assert!(speed.contains(&0));
        assert!(speed.contains(&250));
        assert!(!speed.contains(&251));
        assert!(!speed.contains(&-1));

        let only_min = Range {
            min: Some(-40.0),
            max: None,
        };
        assert!(only_min.contains(&1e9));
        assert!(!only_min.contains(&-40.5));

        // Works for non-Copy types too (only PartialOrd is required).
        let names = Range {
            min: Some("B".to_string()),
            max: Some("D".to_string()),
        };
        assert!(names.contains(&"C".to_string()));
        assert!(!names.contains(&"E".to_string()));
    }

    #[test]
    fn clamp_in_place() {
        let mut v = [-5, 0, 50, 300];
        clamp_all(&mut v, 0, 255);
        assert_eq!(v, [0, 0, 50, 255]);
        let mut f = [0.5, 1.5];
        clamp_all(&mut f, 0.0, 1.0);
        assert_eq!(f, [0.5, 1.0]);
    }
}
