// c15_e05_function_pointers_vs_closures.rs
//
// CONCEPT: `fn` pointers vs closure types vs `impl Fn` vs `Box<dyn Fn>`
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Function pointers, closures and Box<dyn Fn>")
// DATABROKER: databroker/src/broker.rs — `with_read_lock(f: impl FnOnce(..))` and
//             `map_entries(f: impl FnMut(..))` accept any closure (static dispatch).
// EXPECT:  tests fail: `todo!()` panics
//
// Four ways to pass behaviour around:
//   fn(f64) -> f64          a plain function pointer: one word, no captured data
//   impl Fn(f64) -> f64     "some closure type", one per closure, known at compile time
//   Box<dyn Fn(f64) -> f64> any closure, on the heap, called through a vtable
//   &dyn Fn(f64) -> f64     the same, borrowed
// A closure that captures nothing coerces to a `fn` pointer. One that
// captures variables does not, because it has data to carry.
//
// YOUR TASK
//   Implement the `todo!()`s. `CONVERSIONS` is a table of `fn` pointers.
//   Do not edit the tests.

// I AM NOT DONE

fn kmh_to_ms(kmh: f64) -> f64 {
    kmh / 3.6
}

/// A plain function pointer. The alias keeps the table's type readable.
type Conversion = fn(f64) -> f64;

/// A lookup table of conversions. Only `fn` pointers fit in a `const`/`static` table.
const CONVERSIONS: &[(&str, Conversion)] = &[
    ("kmh_to_ms", kmh_to_ms),
    ("c_to_f", |c| c * 9.0 / 5.0 + 32.0),
    ("identity", |x| x),
];

fn convert(name: &str, value: f64) -> Option<f64> {
    todo!()
}

/// Returns a closure that multiplies by `factor`. It captures `factor`, so it
/// can't be a `fn` pointer; `impl Fn` names "some closure type".
fn make_scaler(factor: f64) -> impl Fn(f64) -> f64 {
    // TODO: return a closure that captures `factor`.
    |_x: f64| -> f64 { todo!() }
}

/// Chains `steps` left to right into one callable. The steps have different
/// closure types, so they must be boxed trait objects.
fn pipeline(steps: Vec<Box<dyn Fn(f64) -> f64>>) -> Box<dyn Fn(f64) -> f64> {
    todo!()
}

/// Calls `f` once per value, letting it keep state between calls.
fn feed(values: &[f64], mut f: impl FnMut(f64)) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_lookup() {
        assert_eq!(convert("kmh_to_ms", 36.0), Some(10.0));
        assert_eq!(convert("c_to_f", 100.0), Some(212.0));
        assert_eq!(convert("nope", 1.0), None);
    }

    #[test]
    fn fn_pointer_is_one_word() {
        assert_eq!(
            std::mem::size_of::<fn(f64) -> f64>(),
            std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn scaler_captures_its_factor() {
        let double = make_scaler(2.0);
        let half = make_scaler(0.5);
        assert_eq!(double(21.0), 42.0);
        assert_eq!(half(double(8.0)), 8.0);
    }

    #[test]
    fn pipeline_runs_steps_in_order() {
        let offset = 1.0;
        let p = pipeline(vec![
            Box::new(kmh_to_ms),        // a fn item, boxed
            Box::new(make_scaler(2.0)), // an impl Fn, boxed
            Box::new(move |x| x + offset),
        ]);
        assert_eq!(p(36.0), 21.0);
        assert_eq!(pipeline(vec![])(5.0), 5.0);
    }

    #[test]
    fn feed_allows_mutable_state() {
        let mut max = f64::MIN;
        feed(&[3.0, 9.0, 4.0], |v| max = max.max(v));
        assert_eq!(max, 9.0);
    }
}
