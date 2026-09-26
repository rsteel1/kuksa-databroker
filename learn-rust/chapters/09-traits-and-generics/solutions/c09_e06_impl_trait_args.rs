// c09_e06_impl_trait_args.rs
//
// CONCEPT: flexible APIs with `impl Trait` in argument position
//          (`impl Into<String>`, `impl AsRef<str>`, `impl IntoIterator<Item = ..>`)
//          and in return position (`-> impl Iterator<Item = ..>`).
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "impl Trait: flexible arguments, opaque returns")
// DATABROKER: databroker/src/broker.rs —
//               `DataBroker::new(version: impl Into<String>, commit_sha: impl Into<String>)`
//               `DatabaseReadAccess::get_entry_by_path(&self, path: impl AsRef<str>)`
//               `AuthorizedAccess::update_entries(&self, updates: impl IntoIterator<Item = (i32, EntryUpdate)>)`
// EXPECT:  compile errors E0308 (mismatched types) and E0283 (type annotations
//          needed) in the tests
//
// `fn f(x: impl Into<String>)` is shorthand for `fn f<S: Into<String>>(x: S)`.
// Callers can pass a `&str`, a `String`, ... — anything convertible. The
// function decides *once* where the conversion happens.
//
// YOUR TASK
//   Change the parameter types (only the signatures!) of `new`, `find`
//   and `update_all` so the tests compile, then implement `paths` returning
//   `impl Iterator<Item = &str> + '_`. Do not edit the tests.

use std::collections::HashMap;

struct Broker {
    version: String,
    values: HashMap<String, f64>,
}

impl Broker {
    fn new(version: impl Into<String>) -> Self {
        Broker {
            version: version.into(),
            values: HashMap::new(),
        }
    }

    fn find(&self, path: impl AsRef<str>) -> Option<f64> {
        self.values.get(path.as_ref()).copied()
    }

    fn update_all(&mut self, updates: impl IntoIterator<Item = (String, f64)>) {
        for (path, value) in updates {
            self.values.insert(path, value);
        }
    }

    /// All known paths, sorted. The caller only knows "some iterator of &str";
    /// the concrete iterator type stays private.
    fn paths(&self) -> impl Iterator<Item = &str> + '_ {
        let mut paths: Vec<&str> = self.values.keys().map(String::as_str).collect();
        paths.sort_unstable();
        paths.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_str_and_string() {
        assert_eq!(Broker::new("0.7.2").version, "0.7.2");
        assert_eq!(Broker::new(String::from("dev")).version, "dev");
    }

    #[test]
    fn update_accepts_any_iterable_and_find_any_string_like() {
        let mut b = Broker::new("v");
        // an array
        b.update_all([("Vehicle.Speed".to_string(), 50.0)]);
        // a Vec
        b.update_all(vec![("Vehicle.Width".to_string(), 1.8)]);
        // a HashMap (iterates as (K, V) pairs)
        b.update_all(HashMap::from([("Vehicle.Speed".to_string(), 60.0)]));
        // a lazy iterator
        b.update_all((1..=2).map(|i| (format!("Vehicle.Row{i}"), i as f64)));

        assert_eq!(b.find("Vehicle.Speed"), Some(60.0));
        assert_eq!(b.find(String::from("Vehicle.Width")), Some(1.8));
        let key = String::from("Vehicle.Row2");
        assert_eq!(b.find(&key), Some(2.0));
        assert_eq!(b.find("nope"), None);
    }

    #[test]
    fn paths_sorted() {
        let mut b = Broker::new("v");
        b.update_all([("B".to_string(), 1.0), ("A".to_string(), 2.0)]);
        assert_eq!(b.paths().collect::<Vec<_>>(), vec!["A", "B"]);
    }
}
