// c16_e03_hrtb.rs
//
// CONCEPT: higher-ranked trait bounds: `for<'x> Fn(&'x str) -> &'x str`
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "Higher-ranked trait bounds (for<'a>)")
// DATABROKER: databroker/src/broker.rs — `filter_map_entries(f: impl FnMut(EntryReadAccess) -> Option<T>)`:
//             the closure is called with borrows created *inside* the method, so its
//             bound has to work for every lifetime (the elided form is higher-ranked).
// EXPECT:  compile errors: E0597 (`trimmed` does not live long enough) and
//          "lifetime may not live long enough" + "implementation of `Fn` is not general
//          enough" (the closure in `segment_fn`)
//
// `F: Fn(&'a str) -> &'a str` with `'a` a parameter of your function means
// "works for ONE lifetime `'a`, chosen by the CALLER". Then you can't pass
// the closure a reference to a local you create inside the function, because
// that local doesn't live for the caller's `'a`.
//
// `F: for<'x> Fn(&'x str) -> &'x str` means "works for EVERY lifetime `'x`".
// That's what you need when *you* create the borrow. `Fn(&str) -> &str` with
// elided lifetimes is exactly this higher-ranked form.
//
// Part 2: a closure stored in a `let` gets its signature inferred on its
// own, and for "takes a `&str`, returns a `&str`" the inference picks two
// unrelated lifetimes. A closure written directly where a higher-ranked
// bound is expected (a function argument or return position) is inferred
// from that bound instead.
//
// YOUR TASK
//   1. Fix the bound on `normalize_all`.
//   2. Give `hr` the higher-ranked bound it's named for, and change
//      `segment_fn` to pass its closure straight into `hr(...)`.
//   Do not edit the tests.

/// Lowercases and trims each input, then applies `f` to the *local* result.
// WHY: `f` is called with a borrow of `trimmed`, a local, so it must accept any lifetime.
fn normalize_all<F>(inputs: &[String], f: F) -> Vec<String>
where
    F: for<'x> Fn(&'x str) -> &'x str,
{
    inputs
        .iter()
        .map(|s| {
            let trimmed = s.trim().to_lowercase();
            f(&trimmed).to_string()
        })
        .collect()
}

/// Identity function whose only job is to give a closure a higher-ranked signature.
fn hr<F>(f: F) -> F
where
    F: for<'x> Fn(&'x str) -> &'x str,
{
    f
}

/// Returns a function that extracts the first `.`-separated segment.
fn segment_fn() -> impl Fn(&str) -> &str {
    // WHY: without `hr`, the closure's return lifetime is inferred separately
    // from its argument's, and the compiler rejects returning `s`'s data.
    hr(|s: &str| s.split('.').next().unwrap_or(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_then_applies() {
        let inputs = vec![
            "  Vehicle.Speed ".to_string(),
            "VEHICLE.Cabin.Temp".to_string(),
        ];
        let out = normalize_all(&inputs, |s| s.strip_prefix("vehicle.").unwrap_or(s));
        assert_eq!(out, ["speed", "cabin.temp"]);
    }

    #[test]
    fn works_with_fn_items() {
        fn last_segment(s: &str) -> &str {
            s.rsplit('.').next().unwrap_or(s)
        }
        let out = normalize_all(&["A.B.C".to_string()], last_segment);
        assert_eq!(out, ["c"]);
    }

    #[test]
    fn closure_returning_a_borrow() {
        let first_segment = segment_fn();
        assert_eq!(first_segment("Vehicle.Speed"), "Vehicle");
        let owned = String::from("Cabin.Door");
        assert_eq!(first_segment(&owned), "Cabin");
    }
}
