// c16_e04_variance.rs
//
// CONCEPT: variance: `&'a T` is covariant, `&'a mut T` is invariant in `T`
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "Variance: when a longer lifetime can stand in for a shorter one")
// DATABROKER: databroker/src/broker.rs — `authorized_write_access<'a, 'b>(&'a mut self,
//             permissions: &'b Permissions)` keeps two independent borrows on two
//             lifetimes. Both bugs here come from forcing independent lifetimes to be equal.
// EXPECT:  compile errors in the tests: E0597 (`owned` does not live long enough) and
//          E0499/E0502 (`current` stays mutably borrowed after `set_slot` returns).
//          The bugs are in the two function *signatures*, not in the tests.
//
// Covariance: a `&'static str` can be used where a `&'short str` is expected
// (it lives *longer*, so that's safe). This is why literals mix freely with
// borrowed strings.
//
// Invariance: behind `&mut`, the pointed-to type must match *exactly*. If a
// function takes `&mut Vec<&'static str>`, you can't give it a
// `&mut Vec<&'short str>`. The function could push a `&'static str`, which is
// fine, but the compiler must also stop you from doing the reverse. So the
// caller's vector is forced to be `Vec<&'static str>`, and then your local
// `String` "does not live long enough".
//
// `&'a mut &'a str` ties the mutable borrow to the *same* lifetime as the
// data, so the variable stays mutably borrowed for as long as the data lives:
// effectively forever.
//
// YOUR TASK
//   Change ONLY the signatures of `add_defaults` and `set_slot` so the tests
//   compile. Bodies and tests stay as they are.

/// Appends the default signals to `paths`.
// WHY: `&mut Vec<&str>` elides to `&'x mut Vec<&'y str>`: two fresh lifetimes.
// Literals (`'static`) shrink to `'y` by covariance, so the caller's vector can
// hold short-lived borrows too.
fn add_defaults(paths: &mut Vec<&str>) {
    paths.push("Vehicle.Speed");
    paths.push("Vehicle.Width");
}

/// Replaces the string that `slot` points at.
// WHY: the `&mut` gets its own (anonymous, short) lifetime. Only the data needs `'a`.
fn set_slot<'a>(slot: &mut &'a str, value: &'a str) {
    *slot = value;
}

/// Covariance in action: a `'static` and a local borrow unify to the shorter one.
fn longer<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_mix_with_borrowed_paths() {
        let owned = String::from("Vehicle.Cabin.Temperature");
        let mut paths: Vec<&str> = vec![&owned];
        add_defaults(&mut paths);
        assert_eq!(
            paths,
            [
                "Vehicle.Cabin.Temperature",
                "Vehicle.Speed",
                "Vehicle.Width"
            ]
        );
    }

    #[test]
    fn slot_can_be_read_after_setting() {
        let mut current: &str = "none";
        let incoming = String::from("Vehicle.Speed");
        set_slot(&mut current, &incoming);
        assert_eq!(current, "Vehicle.Speed");
        set_slot(&mut current, "literal");
        assert_eq!(current, "literal");
    }

    #[test]
    fn covariance_lets_static_shrink() {
        let local = String::from("Vehicle.Cabin");
        let result = longer("Vehicle.Speed.Current", &local);
        assert_eq!(result, "Vehicle.Speed.Current");
    }
}
