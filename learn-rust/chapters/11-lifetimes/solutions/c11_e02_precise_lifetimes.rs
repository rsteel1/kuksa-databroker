// c11_e02_precise_lifetimes.rs
//
// CONCEPT: tying an output to the RIGHT input — and knowing when you can't
//          return a reference at all (return an owned value instead).
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Choosing which input the output borrows from")
// DATABROKER: databroker/src/broker.rs — `DatabaseReadAccess::get_entry_by_path`
//          takes `path: impl AsRef<str>` (a short-lived key) but returns
//          `&Entry` borrowed from the database, not from the key.
// EXPECT:  compile errors E0106 (missing lifetime specifier) and
//          E0515 (cannot return reference to local variable).
//
// If you give two inputs the same lifetime, the output is only valid while
// BOTH are alive. That's too strict for a lookup: the search key is usually a
// temporary, but the entry lives in the database for much longer.
//
// YOUR TASK
//   1. Give `lookup` a lifetime so the returned entry borrows only from `db`.
//      (One of the tests uses a temporary key — it must compile.)
//   2. `normalized` creates a new string. You cannot return a reference to
//      something created inside the function. Change its return type.
//   Do not edit the tests.

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
struct Entry {
    id: i32,
    value: f32,
}

// WHY: only `db` carries 'db; `path` gets its own (elided) lifetime, so a
// temporary key may die while the returned entry is still in use.
fn lookup<'db>(db: &'db HashMap<String, Entry>, path: &str) -> Option<&'db Entry> {
    db.get(path)
}

/// Accepts '/'-separated paths ("Vehicle/Speed") and returns the '.' form.
// WHY: `dotted` is created here and dropped at the closing brace; the caller
// must own it, so return the String itself.
fn normalized(path: &str) -> String {
    path.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> HashMap<String, Entry> {
        HashMap::from([("Vehicle.Speed".to_string(), Entry { id: 1, value: 42.0 })])
    }

    #[test]
    fn lookup_with_a_temporary_key() {
        let db = db();
        let entry = {
            let key = format!("Vehicle.{}", "Speed"); // dies at the end of this block
            lookup(&db, &key)
        };
        assert_eq!(entry, Some(&Entry { id: 1, value: 42.0 }));
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalized("Vehicle/Cabin/Temp"), "Vehicle.Cabin.Temp");
        assert_eq!(normalized("Vehicle.Speed"), "Vehicle.Speed");
    }
}
