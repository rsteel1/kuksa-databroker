// c08_e03_visibility_pub_crate.rs
//
// CONCEPT: restricted visibility: `pub(crate)`, `pub(super)`, `pub(in path)`.
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Restricted visibility")
// DATABROKER: databroker/src/grpc/mod.rs — `pub mod server; mod kuksa_val_v1;
//             mod kuksa_val_v2;`: the API implementations are private modules,
//             yet their `impl Val for DataBroker` blocks work everywhere
//             (trait impls are not subject to module privacy).
// EXPECT:  compile error E0603 (function `raw_insert` is private) and
//          E0616 (field `next_id` of struct `Database` is private)
//
// `pub` means "visible to anyone who can see my parent". Often you want
// something in between private and public:
//   pub(super)       visible in the parent module
//   pub(crate)       visible anywhere in this crate, but not to other crates
//   pub(in crate::a) visible inside crate::a and below
// Use the *narrowest* visibility that works: it documents intent and keeps
// refactors local.
//
// YOUR TASK
//   Make the code compile with the most restrictive visibility possible:
//     - `raw_insert` is only called from the parent module `broker`
//       -> pub(super)
//     - `Database::next_id` is read by the `grpc` module (a different branch
//       of the tree) -> pub(crate)
//   Do not make anything plain `pub` that isn't already. Do not edit the tests.

// I AM NOT DONE

mod broker {
    pub mod database {
        use std::collections::HashMap;

        #[derive(Default)]
        pub struct Database {
            next_id: i32,
            entries: HashMap<String, i32>,
        }

        /// Low-level insert with no checks. Only `broker::add` should call it.
        fn raw_insert(db: &mut Database, path: &str) -> i32 {
            let id = db.next_id;
            db.entries.insert(path.to_string(), id);
            db.next_id += 1;
            id
        }

        impl Database {
            pub fn lookup(&self, path: &str) -> Option<i32> {
                self.entries.get(path).copied()
            }
        }
    }

    /// Public API: validates, then inserts.
    pub fn add(db: &mut database::Database, path: &str) -> Result<i32, String> {
        if !path.starts_with("Vehicle.") {
            return Err(format!("invalid path: {path}"));
        }
        if let Some(id) = db.lookup(path) {
            return Ok(id);
        }
        Ok(database::raw_insert(db, path))
    }
}

mod grpc {
    use crate::broker::database::Database;

    /// A server-info style report reaching into broker internals.
    pub fn server_info(db: &Database) -> String {
        format!("{} signals registered", db.next_id)
    }
}

#[cfg(test)]
mod tests {
    use super::broker::{self, database::Database};
    use super::grpc;

    #[test]
    fn add_goes_through_validation() {
        let mut db = Database::default();
        assert_eq!(broker::add(&mut db, "Vehicle.Speed"), Ok(0));
        assert_eq!(broker::add(&mut db, "Vehicle.Speed"), Ok(0));
        assert_eq!(broker::add(&mut db, "Vehicle.Width"), Ok(1));
        assert!(broker::add(&mut db, "Car.Speed").is_err());
        assert_eq!(db.lookup("Vehicle.Width"), Some(1));
    }

    #[test]
    fn grpc_can_read_crate_visible_field() {
        let mut db = Database::default();
        broker::add(&mut db, "Vehicle.A").unwrap();
        broker::add(&mut db, "Vehicle.B").unwrap();
        assert_eq!(grpc::server_info(&db), "2 signals registered");
    }
}
