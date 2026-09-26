// c11_e04_enum_holding_refs.rs
//
// CONCEPT: enums whose variants hold references, `impl<'a>` blocks, and
//          the anonymous lifetime `'_`.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Enums that borrow, and '_")
// DATABROKER: databroker/src/broker.rs — `pub enum EntryReadAccess<'a> {
//          Entry(&'a Entry), Err(&'a Metadata, ReadError) }` (line 1072) and
//          its `impl EntryReadAccess<'_>` / `impl<'a> EntryReadAccess<'a>`.
// EXPECT:  tests fail: `todo!()` panics.
//
// When the databroker iterates all entries for a caller, it doesn't skip
// unreadable ones — it yields `EntryReadAccess::Err(&metadata, error)` so the
// caller can still see *that* the signal exists (metadata is public) but not
// its value. Both variants borrow from the same database: one lifetime `'a`.
//
// YOUR TASK
//   Implement `EntryReadAccess::new`, `datapoint`, and `metadata`.
//   Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
struct Metadata {
    path: String,
    unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct Datapoint {
    value: f64,
}

#[derive(Debug)]
struct Entry {
    metadata: Metadata,
    datapoint: Datapoint,
}

#[derive(Debug, Clone, PartialEq)]
enum ReadError {
    PermissionDenied,
    PermissionExpired,
}

struct Permissions {
    readable_prefix: String,
    expired: bool,
}

impl Permissions {
    fn can_read(&self, path: &str) -> Result<(), ReadError> {
        if self.expired {
            Err(ReadError::PermissionExpired)
        } else if path.starts_with(&self.readable_prefix) {
            Ok(())
        } else {
            Err(ReadError::PermissionDenied)
        }
    }
}

enum EntryReadAccess<'a> {
    Entry(&'a Entry),
    Err(&'a Metadata, ReadError),
}

impl<'a> EntryReadAccess<'a> {
    /// Checks `permissions` for this entry's path and picks the variant.
    fn new(entry: &'a Entry, permissions: &Permissions) -> Self {
        todo!()
    }
}

// `'_` = "some lifetime, I don't need to name it here".
impl EntryReadAccess<'_> {
    /// The datapoint if readable, otherwise a clone of the stored error.
    fn datapoint(&self) -> Result<&Datapoint, ReadError> {
        todo!()
    }

    /// Metadata is always available.
    fn metadata(&self) -> &Metadata {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry {
            metadata: Metadata {
                path: path.into(),
                unit: Some("km/h".into()),
            },
            datapoint: Datapoint { value: 88.0 },
        }
    }

    #[test]
    fn readable_entry() {
        let e = entry("Vehicle.Speed");
        let perms = Permissions {
            readable_prefix: "Vehicle".into(),
            expired: false,
        };
        let access = EntryReadAccess::new(&e, &perms);
        assert_eq!(access.datapoint(), Ok(&Datapoint { value: 88.0 }));
        assert_eq!(access.metadata().path, "Vehicle.Speed");
    }

    #[test]
    fn denied_entry_still_has_metadata() {
        let e = entry("Private.Thing");
        let perms = Permissions {
            readable_prefix: "Vehicle".into(),
            expired: false,
        };
        let access = EntryReadAccess::new(&e, &perms);
        assert_eq!(access.datapoint(), Err(ReadError::PermissionDenied));
        assert_eq!(access.metadata().unit.as_deref(), Some("km/h"));
    }

    #[test]
    fn expired_permissions() {
        let e = entry("Vehicle.Speed");
        let perms = Permissions {
            readable_prefix: "Vehicle".into(),
            expired: true,
        };
        let access = EntryReadAccess::new(&e, &perms);
        assert_eq!(access.datapoint(), Err(ReadError::PermissionExpired));
    }

    #[test]
    fn works_over_many_entries() {
        let entries = [entry("Vehicle.A"), entry("Other.B"), entry("Vehicle.C")];
        let perms = Permissions {
            readable_prefix: "Vehicle".into(),
            expired: false,
        };
        let readable = entries
            .iter()
            .map(|e| EntryReadAccess::new(e, &perms))
            .filter(|a| a.datapoint().is_ok())
            .count();
        assert_eq!(readable, 2);
    }
}
