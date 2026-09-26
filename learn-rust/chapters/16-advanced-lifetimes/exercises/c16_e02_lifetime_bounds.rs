// c16_e02_lifetime_bounds.rs
//
// CONCEPT: lifetime bounds on trait objects (`dyn Trait + 'a`) and the default object lifetime
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "Lifetime bounds and trait-object lifetimes")
// DATABROKER: databroker/src/broker.rs — `Arc<dyn ActuationProvider + Send + Sync + 'static>`
//             spells out `'static`. `Box<dyn SignalProvider>` gets it implicitly (the default).
// EXPECT:  compile errors: E0107 (the tests use `CheckSet<'static>`, but `CheckSet` has no
//          lifetime parameter yet), "lifetime may not live long enough" in
//          `make_prefix_check`, and E0373 "closure may outlive the current function"
//          (the boxed closures borrow data, but the types demand `'static`)
//
// A trait object hides the concrete type, including any references inside it.
// So the type must say how long those hidden references are valid:
//   Box<dyn Fn(&str) -> bool>        means  Box<dyn Fn(&str) -> bool + 'static>
//   &'a dyn Fn(&str) -> bool         means  &'a (dyn Fn(&str) -> bool + 'a)
// A closure that borrows a local `&str` is not `'static`, so it doesn't fit
// in the first. Write `+ 'a` (or `+ '_`) to allow shorter borrows.
//
// YOUR TASK
//   Add lifetime parameters/bounds so `make_prefix_check`, the `PathCheck`
//   alias and `CheckSet` accept closures that borrow their inputs. Don't add
//   `.to_string()`/`clone()` to dodge the borrow: that would hide the lesson.
//   Do not edit the tests.

// I AM NOT DONE

/// Returns a check that borrows `prefix` instead of copying it.
fn make_prefix_check(prefix: &str) -> Box<dyn Fn(&str) -> bool> {
    Box::new(move |path| path.starts_with(prefix))
}

/// A boxed path check that may borrow data living at least as long as `'a`.
type PathCheck = Box<dyn Fn(&str) -> bool>;

/// A set of path checks that may borrow data living at least as long as `'a`.
struct CheckSet {
    checks: Vec<PathCheck>,
}

impl CheckSet {
    fn new() -> Self {
        CheckSet { checks: Vec::new() }
    }

    fn add(&mut self, check: impl Fn(&str) -> bool + 'static) {
        self.checks.push(Box::new(check));
    }

    /// True if every check accepts `path`.
    fn accepts(&self, path: &str) -> bool {
        self.checks.iter().all(|check| check(path))
    }
}

/// A struct holding a reference to `T` needs `T: 'a`: the referent must be
/// valid for as long as the reference. Since 2018 the compiler infers this
/// bound on structs; it's written out here so you can see it.
struct Borrowed<'a, T: 'a> {
    inner: &'a T,
}

impl<'a, T: 'a + PartialEq> Borrowed<'a, T> {
    fn is(&self, other: &T) -> bool {
        self.inner == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_check_borrows() {
        let prefix = String::from("Vehicle.Cabin.");
        let check = make_prefix_check(&prefix);
        assert!(check("Vehicle.Cabin.Temperature"));
        assert!(!check("Vehicle.Speed"));
    }

    #[test]
    fn check_set_holds_borrowing_closures() {
        let banned = ["Vehicle.Private".to_string()];
        let root = String::from("Vehicle.");
        let mut set = CheckSet::new();
        set.add(|p| p.starts_with(root.as_str()));
        set.add(|p| !banned.iter().any(|b| p.starts_with(b.as_str())));
        set.add(make_prefix_check(&root));
        assert!(set.accepts("Vehicle.Speed"));
        assert!(!set.accepts("Vehicle.Private.Key"));
        assert!(!set.accepts("Other.Speed"));
    }

    #[test]
    fn static_closures_still_fit() {
        let mut set: CheckSet<'static> = CheckSet::new();
        set.add(|p: &str| !p.is_empty());
        assert!(set.accepts("x"));
    }

    #[test]
    fn borrowed_wrapper() {
        let value = 42;
        let b = Borrowed { inner: &value };
        assert!(b.is(&42));
    }
}
