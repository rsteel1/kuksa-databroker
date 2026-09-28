// c14_e10_review_trait_bugs.rs
//
// CONCEPT: code review. Three independent pieces of code, each compiling
//          cleanly, each with one planted bug typical of advanced-trait
//          code: an inherent method silently shadowing a trait method, a
//          `Borrow` impl that breaks the map-lookup contract it exists for,
//          and a `dyn Any` downcast checking the wrong concrete type.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Deref, AsRef and Borrow")
// DATABROKER: databroker/src/glob.rs and `broker.rs`'s `HashMap<String, ..>`
//          lookups rely on `Borrow<str>` for `String` keys working
//          correctly — a broken `Borrow` impl like bug 2 would make every
//          lookup by `&str` silently fail.
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests. Bug 1 requires
//   deleting a method; bug 2 and bug 3 are one-line fixes. For each bug,
//   add a one-line comment directly above your fix: `// BUG 1: <what was
//   wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

use std::any::Any;
use std::borrow::Borrow;
use std::collections::HashMap;

// --- Bug 1: an inherent method shadows the trait method -------------------

trait Describe {
    fn describe(&self) -> String {
        "unknown".to_string()
    }
}

struct Sensor {
    name: String,
}

impl Sensor {
    fn describe(&self) -> String {
        "sensor".to_string()
    }
}

impl Describe for Sensor {
    fn describe(&self) -> String {
        format!("Sensor({})", self.name)
    }
}

// --- Bug 2: a Borrow impl that doesn't actually borrow the key -----------

#[derive(Debug, Eq, PartialEq, Hash)]
struct Path(String);

impl Borrow<str> for Path {
    fn borrow(&self) -> &str {
        ""
    }
}

fn lookup<'a>(map: &'a HashMap<Path, i32>, key: &str) -> Option<&'a i32> {
    map.get(key)
}

// --- Bug 3: downcasting to the wrong concrete type -------------------------

/// Doubles the boxed value in place if it holds an `i64`. Returns whether
/// it did.
fn double_if_i64(value: &mut Box<dyn Any>) -> bool {
    if let Some(n) = value.downcast_mut::<i32>() {
        *n *= 2;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_uses_the_trait_impl_not_the_inherent_shadow() {
        let sensor = Sensor {
            name: "main".to_string(),
        };
        assert_eq!(sensor.describe(), "Sensor(main)");
    }

    #[test]
    fn lookup_by_str_finds_a_path_key() {
        let mut map = HashMap::new();
        map.insert(Path("Vehicle.Speed".to_string()), 42);
        assert_eq!(lookup(&map, "Vehicle.Speed"), Some(&42));
        assert_eq!(lookup(&map, "Vehicle.Unknown"), None);
    }

    #[test]
    fn double_if_i64_doubles_an_i64_and_reports_true() {
        let mut value: Box<dyn Any> = Box::new(5i64);
        assert!(double_if_i64(&mut value));
        assert_eq!(*value.downcast_ref::<i64>().unwrap(), 10);
    }

    #[test]
    fn double_if_i64_leaves_other_types_alone() {
        let mut value: Box<dyn Any> = Box::new("hello".to_string());
        assert!(!double_if_i64(&mut value));
        assert_eq!(value.downcast_ref::<String>().unwrap(), "hello");
    }
}
