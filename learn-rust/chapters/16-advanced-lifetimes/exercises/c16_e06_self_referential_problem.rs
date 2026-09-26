// c16_e06_self_referential_problem.rs
//
// CONCEPT: a struct can't hold references into itself; use indices (or ids) instead
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "The self-referential struct problem")
// DATABROKER: databroker/src/broker.rs — `struct Database { next_id, path_to_id:
//             HashMap<String, i32>, entries: HashMap<i32, Entry> }`. The path index stores
//             *ids*, not `&Entry`s. This exercise shows why it has to.
// EXPECT:  compile errors in the tests: E0499 (cannot borrow `db` as mutable more than
//          once at a time) and E0502
//
// The shipped `Database<'a>` wants a fast path index: `HashMap<&'a str, &'a Entry>`
// pointing into its own `entries` vector. To make that type-check, `add` has
// to take `&'a mut self`, which borrows the database mutably *for its whole
// lifetime*. One call to `add` and the database is locked forever.
//
// This isn't the borrow checker being fussy. If `entries` grows, the `Vec`
// reallocates and every `&Entry` in the index would dangle.
//
// The standard fixes, in order of preference:
//   1. store indices or ids instead of references (what the databroker does);
//   2. share ownership with `Arc<Entry>` in both places (costs a refcount);
//   3. pin the data and use unsafe code or a crate like `ouroboros` (rarely worth it;
//      chapter 24 explains `Pin`).
//
// YOUR TASK
//   Redesign `Database` with fix 1: no lifetime parameter, a
//   `path_to_id: HashMap<String, usize>` index into `entries`. Keep the public
//   method names and behaviour that the tests use.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    path: String,
    value: i64,
}

#[derive(Debug, PartialEq)]
enum DbError {
    AlreadyExists,
    NotFound,
}

struct Database<'a> {
    entries: Vec<Entry>,
    // A fast index pointing straight at the entries. Looks reasonable...
    by_path: HashMap<&'a str, &'a Entry>,
}

impl<'a> Database<'a> {
    fn new() -> Self {
        Database {
            entries: Vec::new(),
            by_path: HashMap::new(),
        }
    }

    /// Adds a signal and returns its id.
    // `&'a mut self`: the only way to store `&'a` references into `self`...
    fn add(&'a mut self, path: &str, value: i64) -> Result<usize, DbError> {
        if self.by_path.contains_key(path) {
            return Err(DbError::AlreadyExists);
        }
        self.entries.push(Entry {
            path: path.to_string(),
            value,
        });
        let entry = self.entries.last().expect("just pushed");
        self.by_path.insert(&entry.path, entry);
        Ok(self.entries.len() - 1)
    }

    fn entry(&self, id: usize) -> Option<&Entry> {
        self.entries.get(id)
    }

    fn entry_by_path(&self, path: &str) -> Option<&Entry> {
        self.by_path.get(path).copied()
    }

    fn set_value(&mut self, path: &str, value: i64) -> Result<(), DbError> {
        // Can't mutate through a shared `&Entry` in the index, and can't find
        // the position in `entries` without searching...
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.path == path)
            .ok_or(DbError::NotFound)?;
        entry.value = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_many_then_read() {
        let mut db = Database::new();
        let speed = db.add("Vehicle.Speed", 0).unwrap();
        let width = db.add("Vehicle.Width", 1800).unwrap();
        assert_ne!(speed, width);
        assert_eq!(db.entry(width).map(|e| e.value), Some(1800));
        assert_eq!(db.entry_by_path("Vehicle.Speed").map(|e| e.value), Some(0));
        assert_eq!(db.entry_by_path("nope"), None);
    }

    #[test]
    fn index_survives_growth() {
        let mut db = Database::new();
        for i in 0..1000 {
            db.add(&format!("Vehicle.Sensor{i}"), i).unwrap(); // forces Vec reallocations
        }
        assert_eq!(
            db.entry_by_path("Vehicle.Sensor0").map(|e| e.value),
            Some(0)
        );
        assert_eq!(
            db.entry_by_path("Vehicle.Sensor999").map(|e| e.value),
            Some(999)
        );
    }

    #[test]
    fn update_and_errors() {
        let mut db = Database::new();
        db.add("Vehicle.Speed", 0).unwrap();
        assert_eq!(db.add("Vehicle.Speed", 5), Err(DbError::AlreadyExists));
        db.set_value("Vehicle.Speed", 120).unwrap();
        assert_eq!(
            db.entry_by_path("Vehicle.Speed").map(|e| e.value),
            Some(120)
        );
        assert_eq!(db.set_value("Vehicle.Nope", 1), Err(DbError::NotFound));
    }
}
