// c09_e07_trait_objects_dyn.rs
//
// CONCEPT: static vs dynamic dispatch; trait objects (`Box<dyn Trait>`,
//          `Arc<dyn Trait>`); object safety (dyn compatibility).
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Static vs dynamic dispatch")
//          and (section "Object safety: which traits can be dyn")
// DATABROKER: databroker/src/broker.rs —
//               `actuation_provider: Arc<dyn ActuationProvider + Send + Sync + 'static>`
//               `signal_provider: Arc<dyn SignalProvider>`
//             The broker stores providers of *different* concrete types (the
//             gRPC Provider, test doubles) in one collection: only a trait
//             object can do that.
// EXPECT:  compile errors: E0782 (trait objects must include the `dyn` keyword)
//          and E0038 (the trait `ActuationProvider` is not dyn compatible)
//
// A generic `fn f<P: Provider>(p: &P)` is compiled once per concrete type:
// fast, but a `Vec<P>` can only hold ONE type. `dyn Provider` is a single
// type-erased type: a pointer to data plus a pointer to a vtable of methods.
// A `Vec<Box<dyn Provider>>` can mix types; each call is an indirect call.
//
// Not every trait can be used as `dyn`: e.g. generic methods can't go in a
// vtable (which instantiation would it contain?).
//
// YOUR TASK
//   1. Fix the missing `dyn` keywords.
//   2. Make `ActuationProvider` dyn compatible WITHOUT removing
//      `log_line`: add `where Self: Sized` to that method (it then just
//      isn't callable through `dyn`, but still works on concrete types).
//   3. Implement `Registry::actuate`: find the first *available* provider
//      that `handles(id)` and forward the call; if none,
//      Err(format!("no provider for {id}")).
//   4. Implement the static-dispatch `actuate_static`.
//   Do not edit the tests.

use std::sync::Arc;

trait ActuationProvider {
    fn handles(&self, id: i32) -> bool;
    fn actuate(&self, id: i32, value: f64) -> Result<String, String>;
    fn is_available(&self) -> bool {
        true
    }
    /// Generic method => there's no single function to put in a vtable.
    fn log_line<S: AsRef<str>>(&self, prefix: S, id: i32) -> String
    where
        Self: Sized,
    {
        let state = if self.is_available() { "up" } else { "down" };
        format!("{} {id} ({state})", prefix.as_ref())
    }
}

struct Seat;
struct Window {
    online: bool,
}
struct Labeled<P> {
    inner: P,
    label: String,
}

impl ActuationProvider for Seat {
    fn handles(&self, id: i32) -> bool {
        (1..=9).contains(&id)
    }
    fn actuate(&self, id: i32, value: f64) -> Result<String, String> {
        Ok(format!("seat {id} -> {value}"))
    }
}

impl ActuationProvider for Window {
    fn handles(&self, id: i32) -> bool {
        (10..=19).contains(&id)
    }
    fn actuate(&self, id: i32, value: f64) -> Result<String, String> {
        Ok(format!("window {id} -> {value}"))
    }
    fn is_available(&self) -> bool {
        self.online
    }
}

impl<P: ActuationProvider> ActuationProvider for Labeled<P> {
    fn handles(&self, id: i32) -> bool {
        self.inner.handles(id)
    }
    fn actuate(&self, id: i32, value: f64) -> Result<String, String> {
        self.inner
            .actuate(id, value)
            .map(|msg| format!("[{}] {msg}", self.label))
    }
    fn is_available(&self) -> bool {
        self.inner.is_available()
    }
}

struct Registry {
    providers: Vec<Arc<dyn ActuationProvider>>,
}

impl Registry {
    fn add(&mut self, provider: Arc<dyn ActuationProvider>) {
        self.providers.push(provider);
    }

    /// Dynamic dispatch: each call goes through the vtable.
    fn actuate(&self, id: i32, value: f64) -> Result<String, String> {
        self.providers
            .iter()
            .find(|p| p.is_available() && p.handles(id))
            .ok_or_else(|| format!("no provider for {id}"))?
            .actuate(id, value)
    }
}

/// Static dispatch: compiled separately for each concrete P.
fn actuate_static<P: ActuationProvider>(
    provider: &P,
    id: i32,
    value: f64,
) -> Result<String, String> {
    if provider.handles(id) {
        provider.actuate(id, value)
    } else {
        Err(format!("no provider for {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        let mut r = Registry {
            providers: Vec::new(),
        };
        r.add(Arc::new(Window { online: false }));
        r.add(Arc::new(Labeled {
            inner: Seat,
            label: "driver".to_string(),
        }));
        r.add(Arc::new(Window { online: true }));
        r
    }

    #[test]
    fn mixed_types_in_one_vec() {
        let r = registry();
        assert_eq!(r.providers.len(), 3);
        assert_eq!(r.actuate(3, 1.0), Ok("[driver] seat 3 -> 1".to_string()));
    }

    #[test]
    fn skips_unavailable_providers() {
        // The first Window is offline; the second one must be used.
        assert_eq!(
            registry().actuate(12, 0.5),
            Ok("window 12 -> 0.5".to_string())
        );
    }

    #[test]
    fn no_provider() {
        assert_eq!(
            registry().actuate(99, 0.0),
            Err("no provider for 99".to_string())
        );
    }

    #[test]
    fn static_dispatch() {
        assert_eq!(actuate_static(&Seat, 1, 2.0), Ok("seat 1 -> 2".to_string()));
        assert_eq!(
            actuate_static(&Window { online: true }, 1, 2.0),
            Err("no provider for 1".to_string())
        );
    }

    #[test]
    fn generic_method_still_works_on_concrete_types() {
        assert_eq!(Seat.log_line("seat", 4), "seat 4 (up)");
        assert_eq!(
            Window { online: false }.log_line("win", 11),
            "win 11 (down)"
        );
    }

    #[test]
    fn a_dyn_pointer_is_two_words() {
        use std::mem::size_of;
        // &Seat is a thin pointer; &dyn is (data pointer, vtable pointer).
        assert_eq!(size_of::<&dyn ActuationProvider>(), 2 * size_of::<&Seat>());
    }
}
