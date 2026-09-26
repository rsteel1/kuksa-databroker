// c11_e05_scoped_authorized_view.rs
//
// CONCEPT: putting it together — shared (`&`) and exclusive (`&mut`) views
//          with two lifetimes, returning borrowed data from them, and
//          `impl Iterator<Item = ...> + '_` return types.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "The databroker's authorized-access pattern")
// DATABROKER: databroker/src/broker.rs — `Database::authorized_read_access` /
//          `authorized_write_access` (line 1351), `DatabaseReadAccess` /
//          `DatabaseWriteAccess` (line 1062), `DatabaseWriteAccess::update`.
// EXPECT:  tests fail: `todo!()` panics.
//
// This is the databroker's central safety idea: you never touch the
// `Database` directly. You ask for a *view* that carries your permissions,
// and every method on the view checks them. Because the view only borrows,
// it costs nothing to create — and the borrow checker guarantees nobody can
// write to the database while any read view exists (and vice versa).
//
// YOUR TASK
//   Implement every `todo!()`. The signatures are complete — read them
//   carefully; the lifetimes tell you where each returned reference points.
//   Do not edit the tests.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    id: i32,
    path: String,
    value: i64,
}

#[derive(Debug, PartialEq)]
enum AccessError {
    NotFound,
    PermissionDenied,
}

struct Permissions {
    read_prefix: String,
    write_prefix: String,
}

impl Permissions {
    fn can_read(&self, path: &str) -> bool {
        path.starts_with(&self.read_prefix)
    }
    fn can_write(&self, path: &str) -> bool {
        path.starts_with(&self.write_prefix)
    }
}

#[derive(Default)]
struct Database {
    path_to_id: HashMap<String, i32>,
    entries: HashMap<i32, Entry>,
}

struct ReadAccess<'db, 'perm> {
    db: &'db Database,
    permissions: &'perm Permissions,
}

struct WriteAccess<'db, 'perm> {
    db: &'db mut Database,
    permissions: &'perm Permissions,
}

impl Database {
    fn add(&mut self, id: i32, path: &str, value: i64) {
        self.path_to_id.insert(path.to_string(), id);
        self.entries.insert(
            id,
            Entry {
                id,
                path: path.to_string(),
                value,
            },
        );
    }

    fn authorized_read_access<'db, 'perm>(
        &'db self,
        permissions: &'perm Permissions,
    ) -> ReadAccess<'db, 'perm> {
        ReadAccess {
            db: self,
            permissions,
        }
    }

    fn authorized_write_access<'db, 'perm>(
        &'db mut self,
        permissions: &'perm Permissions,
    ) -> WriteAccess<'db, 'perm> {
        WriteAccess {
            db: self,
            permissions,
        }
    }
}

impl<'db> ReadAccess<'db, '_> {
    /// NotFound if the path is unknown, PermissionDenied if not readable.
    fn entry_by_path(&self, path: &str) -> Result<&'db Entry, AccessError> {
        // WHY: copy the `&'db Database` out of self so everything we borrow
        // from it carries 'db, not the (shorter) lifetime of `&self`.
        let db: &'db Database = self.db;
        let id = db.path_to_id.get(path).ok_or(AccessError::NotFound)?;
        let entry = db.entries.get(id).ok_or(AccessError::NotFound)?;
        if self.permissions.can_read(&entry.path) {
            Ok(entry)
        } else {
            Err(AccessError::PermissionDenied)
        }
    }

    /// Paths of all entries the caller may read, in any order.
    /// The iterator borrows from the database AND from `self` (for the
    /// permissions) — that's what `+ '_` says.
    fn readable_paths(&self) -> impl Iterator<Item = &'db str> + '_ {
        // `self.db.entries.values()` yields `&'db Entry`.
        self.db
            .entries
            .values()
            .filter(|entry| self.permissions.can_read(&entry.path))
            .map(|entry| entry.path.as_str())
    }
}

impl WriteAccess<'_, '_> {
    /// NotFound if the path is unknown, PermissionDenied if not writable.
    /// Otherwise sets the value and returns the previous one.
    fn update_by_path(&mut self, path: &str, value: i64) -> Result<i64, AccessError> {
        let id = *self.db.path_to_id.get(path).ok_or(AccessError::NotFound)?;
        let entry = self.db.entries.get_mut(&id).ok_or(AccessError::NotFound)?;
        if !self.permissions.can_write(&entry.path) {
            return Err(AccessError::PermissionDenied);
        }
        Ok(std::mem::replace(&mut entry.value, value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        let mut db = Database::default();
        db.add(1, "Vehicle.Speed", 50);
        db.add(2, "Vehicle.Horn", 0);
        db.add(3, "Private.Pin", 1234);
        db
    }

    fn perms() -> Permissions {
        Permissions {
            read_prefix: "Vehicle.".into(),
            write_prefix: "Vehicle.Horn".into(),
        }
    }

    #[test]
    fn read_access() {
        let db = db();
        let perms = perms();
        let read = db.authorized_read_access(&perms);
        assert_eq!(read.entry_by_path("Vehicle.Speed").map(|e| e.value), Ok(50));
        assert_eq!(
            read.entry_by_path("Private.Pin"),
            Err(AccessError::PermissionDenied)
        );
        assert_eq!(read.entry_by_path("Nope"), Err(AccessError::NotFound));

        let mut paths: Vec<&str> = read.readable_paths().collect();
        paths.sort();
        assert_eq!(paths, vec!["Vehicle.Horn", "Vehicle.Speed"]);
    }

    #[test]
    fn write_access() {
        let mut db = db();
        let perms = perms();
        {
            let mut write = db.authorized_write_access(&perms);
            assert_eq!(write.update_by_path("Vehicle.Horn", 1), Ok(0));
            assert_eq!(
                write.update_by_path("Vehicle.Speed", 99),
                Err(AccessError::PermissionDenied)
            );
            assert_eq!(write.update_by_path("Nope", 1), Err(AccessError::NotFound));
        } // write view dropped here: the &mut borrow of `db` ends

        let read = db.authorized_read_access(&perms);
        assert_eq!(read.entry_by_path("Vehicle.Horn").map(|e| e.value), Ok(1));
        assert_eq!(read.entry_by_path("Vehicle.Speed").map(|e| e.value), Ok(50));
    }

    #[test]
    fn entries_outlive_the_read_view() {
        let db = db();
        let entry = {
            let perms = perms();
            db.authorized_read_access(&perms)
                .entry_by_path("Vehicle.Speed")
                .expect("readable")
        };
        assert_eq!(entry.id, 1);
    }
}
