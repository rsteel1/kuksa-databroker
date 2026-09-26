// c14_e02_supertraits_and_blanket_impls.rs
//
// CONCEPT: supertraits (`trait Provider: Named`) and blanket impls
//          (`impl<T: Display> Describe for T`).
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Supertraits and blanket impls")
// DATABROKER: databroker/src/broker.rs — `pub trait SignalProvider: Send + Sync + 'static`
//             is a trait with supertraits: every provider MUST also be
//             thread-safe, so the broker can share it across tasks.
// EXPECT:  compile errors E0277 (`Gps: Named` is not satisfied) and E0599
//          (no method named `describe_line` found for type `i32`).
//
// `trait Provider: Named` means "to be a Provider you must first be Named".
// Inside Provider's default methods you may call `self.name()`.
// A blanket impl implements a trait for EVERY type meeting a bound — this is
// how std gives every `T: Display` a `.to_string()` method.
//
// YOUR TASK
//   1. Make `Gps` satisfy `Provider` (it needs the supertrait too). Its name
//      is "gps". Do not override `describe`.
//   2. Write ONE blanket impl so that every type implementing `Display` gets
//      `describe_line()`, returning "<value> (<len> chars)" where len counts
//      characters of the displayed text.
//   Do not edit the tests.

use std::fmt::Display;

trait Named {
    fn name(&self) -> String;
}

/// A provider must be Named; it gets `describe` for free.
trait Provider: Named {
    fn signals(&self) -> Vec<&'static str>;

    fn describe(&self) -> String {
        format!(
            "provider {} ({} signals)",
            self.name(),
            self.signals().len()
        )
    }
}

struct Gps;

impl Provider for Gps {
    fn signals(&self) -> Vec<&'static str> {
        vec![
            "Vehicle.CurrentLocation.Latitude",
            "Vehicle.CurrentLocation.Longitude",
        ]
    }
}

impl Named for Gps {
    fn name(&self) -> String {
        "gps".to_string()
    }
}

trait Describe {
    fn describe_line(&self) -> String;
}

// WHY: one impl covers i32, f64, &str, String and every future Display type.
impl<T: Display> Describe for T {
    fn describe_line(&self) -> String {
        let text = self.to_string();
        let len = text.chars().count();
        format!("{text} ({len} chars)")
    }
}

/// A trait object of the subtrait can still call supertrait methods.
fn all_names(providers: &[Box<dyn Provider>]) -> Vec<String> {
    providers.iter().map(|p| p.name()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_gets_default_describe() {
        assert_eq!(Gps.describe(), "provider gps (2 signals)");
        let providers: Vec<Box<dyn Provider>> = vec![Box::new(Gps)];
        assert_eq!(all_names(&providers), vec!["gps".to_string()]);
    }

    #[test]
    fn blanket_impl_covers_every_display_type() {
        assert_eq!(42.describe_line(), "42 (2 chars)");
        assert_eq!(1.5_f64.describe_line(), "1.5 (3 chars)");
        assert_eq!("Größe".describe_line(), "Größe (5 chars)");
        assert_eq!(String::from("km/h").describe_line(), "km/h (4 chars)");
    }
}
