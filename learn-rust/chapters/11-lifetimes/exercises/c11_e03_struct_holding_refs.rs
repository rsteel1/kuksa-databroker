// c11_e03_struct_holding_refs.rs
//
// CONCEPT: structs that hold references need lifetime parameters, and methods
//          can return references that outlive the struct itself.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Structs that borrow")
// DATABROKER: databroker/src/broker.rs — `pub struct DatabaseReadAccess<'a, 'b> {
//          db: &'a Database, permissions: &'b Permissions }` (line 1062).
// EXPECT:  compile errors E0106 (missing lifetime specifier) and E0107 on
//          the struct; once that's fixed, E0597 ("`view` does not live long
//          enough") inside a test until `get`'s signature is right.
//
// `ReadView` is a cheap, short-lived "lens" over a database plus the caller's
// permissions. It owns nothing. Its lifetime parameters describe what it
// borrows: the view cannot outlive either borrowed value.
//
// But the *entries* it hands out live in the database, not in the view. If
// `get` returns `&Entry` with the elided lifetime (borrowed from `&self`),
// the entry can't be used after the view is gone — which is needlessly
// strict. Name the database lifetime in the return type instead.
//
// YOUR TASK
//   1. Add lifetime parameters to `ReadView` (suggested names: 'db, 'perm).
//   2. Write the matching `impl<...> ReadView<...>` header.
//   3. Make `get` return an entry borrowed from the database ('db), not from
//      the view.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
struct Entry {
    path: String,
    value: i32,
}

struct Database {
    entries: HashMap<String, Entry>,
}

struct Permissions {
    readable_prefix: String,
}

#[derive(Debug, PartialEq)]
enum ReadError {
    NotFound,
    PermissionDenied,
}

struct ReadView {
    db: &Database,
    permissions: &Permissions,
}

impl Database {
    fn read_view<'db, 'perm>(&'db self, permissions: &'perm Permissions) -> ReadView<'db, 'perm> {
        ReadView {
            db: self,
            permissions,
        }
    }
}

impl ReadView {
    fn get(&self, path: &str) -> Result<&Entry, ReadError> {
        let entry = self.db.entries.get(path).ok_or(ReadError::NotFound)?;
        if entry.path.starts_with(&self.permissions.readable_prefix) {
            Ok(entry)
        } else {
            Err(ReadError::PermissionDenied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        let mut entries = HashMap::new();
        for (path, value) in [("Vehicle.Speed", 50), ("Private.Pin", 1234)] {
            entries.insert(
                path.to_string(),
                Entry {
                    path: path.to_string(),
                    value,
                },
            );
        }
        Database { entries }
    }

    #[test]
    fn reads_and_denies() {
        let db = db();
        let perms = Permissions {
            readable_prefix: "Vehicle.".into(),
        };
        let view = db.read_view(&perms);
        assert_eq!(view.get("Vehicle.Speed").map(|e| e.value), Ok(50));
        assert_eq!(view.get("Private.Pin"), Err(ReadError::PermissionDenied));
        assert_eq!(view.get("Nope"), Err(ReadError::NotFound));
    }

    #[test]
    fn entry_outlives_view_and_permissions() {
        let db = db();
        let entry = {
            // Both the permissions and the view die at the end of this block…
            let perms = Permissions {
                readable_prefix: "Vehicle.".into(),
            };
            let view = db.read_view(&perms);
            view.get("Vehicle.Speed").expect("readable")
        };
        // …but the entry borrows from `db`, which is still alive.
        assert_eq!(entry.value, 50);
    }
}
