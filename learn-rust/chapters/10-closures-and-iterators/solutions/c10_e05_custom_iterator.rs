// c10_e05_custom_iterator.rs
//
// CONCEPT: implementing `Iterator` yourself by wrapping another iterator —
//          the `type Item`, `next`, and `size_hint`.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Writing your own iterator")
// DATABROKER: databroker/src/broker.rs — `EntryReadIterator` wraps
//          `hash_map::Values<'a, i32, Entry>` and turns each `&Entry` into an
//          `EntryReadAccess` that says whether the caller may read it.
// EXPECT:  tests fail: `todo!()` panics.
//
// The lifetimes (`'a`, `'b`) are fully written out for you here; chapter 11
// explains them. For now read `'a` as "borrowed from the database" and `'b`
// as "borrowed from the permissions".
//
// YOUR TASK
//   1. Implement `next` and `size_hint` for `EntryReadIter`.
//   2. Implement `Database::iter_entries`.
//   Do not edit the tests.

use std::collections::{hash_map, HashMap};

struct Entry {
    path: String,
    value: i32,
}

/// Only paths starting with `readable_prefix` may be read.
struct Permissions {
    readable_prefix: String,
}

impl Permissions {
    fn can_read(&self, path: &str) -> bool {
        path.starts_with(&self.readable_prefix)
    }
}

/// What the iterator yields: either the entry, or just its path when the
/// caller isn't allowed to see the value.
#[derive(Debug, PartialEq)]
enum EntryRead<'a> {
    Entry(&'a str, i32),
    Denied(&'a str),
}

struct EntryReadIter<'a, 'b> {
    inner: hash_map::Values<'a, i32, Entry>,
    permissions: &'b Permissions,
}

impl<'a> Iterator for EntryReadIter<'a, '_> {
    type Item = EntryRead<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // WHY: `?`-free version of the databroker's `.next().map(...)`:
        // when `inner` is exhausted it returns None and so do we.
        self.inner.next().map(|entry| {
            if self.permissions.can_read(&entry.path) {
                EntryRead::Entry(&entry.path, entry.value)
            } else {
                EntryRead::Denied(&entry.path)
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

struct Database {
    entries: HashMap<i32, Entry>,
}

impl Database {
    fn iter_entries<'b>(&self, permissions: &'b Permissions) -> EntryReadIter<'_, 'b> {
        EntryReadIter {
            inner: self.entries.values(),
            permissions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        let mut entries = HashMap::new();
        entries.insert(
            1,
            Entry {
                path: "Vehicle.Speed".into(),
                value: 50,
            },
        );
        entries.insert(
            2,
            Entry {
                path: "Vehicle.Cabin.Temp".into(),
                value: 21,
            },
        );
        entries.insert(
            3,
            Entry {
                path: "Private.Secret".into(),
                value: 7,
            },
        );
        Database { entries }
    }

    #[test]
    fn yields_every_entry() {
        let db = db();
        let perms = Permissions {
            readable_prefix: "Vehicle.".into(),
        };
        assert_eq!(db.iter_entries(&perms).count(), 3);
        assert_eq!(db.iter_entries(&perms).size_hint(), (3, Some(3)));
    }

    #[test]
    fn denies_unreadable_entries() {
        let db = db();
        let perms = Permissions {
            readable_prefix: "Vehicle.".into(),
        };
        let mut readable: Vec<(&str, i32)> = db
            .iter_entries(&perms)
            .filter_map(|e| match e {
                EntryRead::Entry(path, value) => Some((path, value)),
                EntryRead::Denied(_) => None,
            })
            .collect();
        readable.sort();
        assert_eq!(
            readable,
            vec![("Vehicle.Cabin.Temp", 21), ("Vehicle.Speed", 50)]
        );
        assert!(db
            .iter_entries(&perms)
            .any(|e| e == EntryRead::Denied("Private.Secret")));
    }
}
