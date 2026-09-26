// c35_e01_database_register.rs
//
// CONCEPT: Capstone step 1 — the signal database: ids, path -> id, metadata.
// GUIDE:   chapters/35-capstone-mini-databroker/guide.md  (section "Step 1: the Database")
// DATABROKER: databroker/src/broker.rs — `struct Database`, `DatabaseWriteAccess::add`,
//             `DatabaseReadAccess::get_entry_by_id/get_entry_by_path`;
//             databroker/src/glob.rs — `is_valid_path`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// Over the six steps of this chapter you build a *mini databroker* whose
// shape mirrors databroker/src/broker.rs. Every step ships with the finished
// code of the step before, so you never carry a broken step forward.
//
// The heart of the broker is a `Database`: every VSS signal
// ("Vehicle.Speed") gets a numeric id when it is registered. Clients can
// look signals up by id (fast, used on the hot path) or by path.
//
// YOUR TASK
//   Implement the four `todo!()`s:
//     * `is_valid_path`          — a path is one or more dot-separated
//                                  segments; a segment is non-empty and has no
//                                  whitespace, ':' or '*'.
//     * `DataValue::matches_type` — NotAvailable fits every type.
//     * `Database::add`          — see the doc comment for the rules.
//     * `Database::get_entry_by_path` — reuse `get_entry_by_id`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Types (compare with databroker/src/types.rs — the real ones have more
// variants: all integer widths and array types)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    NotAvailable,
    Bool(bool),
    Int32(i32),
    Float(f32),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Bool,
    Int32,
    Float,
    String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Sensor,
    Attribute,
    Actuator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Only notify subscribers when the value actually changes.
    OnChange,
    /// Every update is interesting, even if the value is the same.
    Continuous,
}

impl DataValue {
    /// Can this value be stored in a signal of type `data_type`?
    pub fn matches_type(&self, data_type: DataType) -> bool {
        todo!("match on (self, data_type)")
    }
}

// ---------------------------------------------------------------------------
// Entries (compare with `Metadata`, `Datapoint`, `Entry` in broker.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Metadata {
    pub id: i32,
    pub path: String,
    pub data_type: DataType,
    pub entry_type: EntryType,
    pub change_type: ChangeType,
    pub description: String,
    pub min: Option<DataValue>,
    pub max: Option<DataValue>,
    /// The real broker stores this as an array `DataValue`; a Vec is simpler.
    pub allowed: Option<Vec<DataValue>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Datapoint {
    pub ts: SystemTime,
    pub value: DataValue,
}

impl Datapoint {
    pub fn now(value: DataValue) -> Self {
        Datapoint {
            ts: SystemTime::now(),
            value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub datapoint: Datapoint,
    pub actuator_target: Option<Datapoint>,
    pub metadata: Metadata,
}

/// Everything needed to register a signal.
///
/// The real `DatabaseWriteAccess::add` takes ten positional arguments and
/// needs `#[allow(clippy::too_many_arguments)]`. A small builder-style struct
/// is the idiomatic alternative.
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub path: String,
    pub data_type: DataType,
    pub entry_type: EntryType,
    pub change_type: ChangeType,
    pub description: String,
    pub min: Option<DataValue>,
    pub max: Option<DataValue>,
    pub allowed: Option<Vec<DataValue>>,
}

impl NewEntry {
    pub fn new(path: impl Into<String>, data_type: DataType, entry_type: EntryType) -> Self {
        NewEntry {
            path: path.into(),
            data_type,
            entry_type,
            change_type: ChangeType::OnChange,
            description: String::new(),
            min: None,
            max: None,
            allowed: None,
        }
    }

    pub fn min(mut self, min: DataValue) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: DataValue) -> Self {
        self.max = Some(max);
        self
    }

    pub fn allowed(mut self, allowed: Vec<DataValue>) -> Self {
        self.allowed = Some(allowed);
        self
    }

    pub fn continuous(mut self) -> Self {
        self.change_type = ChangeType::Continuous;
        self
    }
}

// ---------------------------------------------------------------------------
// Errors (same names as broker.rs; some variants are only used in later steps)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum RegistrationError {
    ValidationError,
    PermissionDenied,
    PermissionExpired,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadError {
    NotFound,
    PermissionDenied,
    PermissionExpired,
}

// ---------------------------------------------------------------------------
// The database
// ---------------------------------------------------------------------------

/// Is `path` a syntactically valid VSS path such as "Vehicle.Cabin.Door"?
pub fn is_valid_path(path: &str) -> bool {
    todo!("split on '.' and check every segment")
}

#[derive(Default)]
pub struct Database {
    next_id: AtomicI32,
    path_to_id: HashMap<String, i32>,
    entries: HashMap<i32, Entry>,
}

impl Database {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a signal and returns its id.
    ///
    /// Rules (the same as the real broker):
    ///   1. an invalid path  -> `Err(RegistrationError::ValidationError)`
    ///   2. a path that is already registered -> `Ok(existing id)`, nothing changes
    ///   3. `allowed` values that don't match `data_type` -> `ValidationError`
    ///   4. otherwise take the next id from `next_id` (`fetch_add`), store the
    ///      entry in both maps. The initial datapoint is `NotAvailable` (use
    ///      `Datapoint::now`) and there is no actuator target.
    pub fn add(&mut self, new: NewEntry) -> Result<i32, RegistrationError> {
        todo!()
    }

    pub fn get_entry_by_id(&self, id: i32) -> Result<&Entry, ReadError> {
        self.entries.get(&id).ok_or(ReadError::NotFound)
    }

    pub fn get_entry_by_path(&self, path: &str) -> Result<&Entry, ReadError> {
        todo!()
    }

    pub fn id_by_path(&self, path: &str) -> Option<i32> {
        self.path_to_id.get(path).copied()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_invalid_paths() {
        for ok in ["Vehicle", "Vehicle.Speed", "Vehicle.Cabin.Door.Row1.IsOpen"] {
            assert!(is_valid_path(ok), "{ok} should be valid");
        }
        for bad in [
            "",
            ".Vehicle",
            "Vehicle.",
            "Vehicle..Speed",
            "Vehicle.Sp eed",
            "Vehicle.*",
            "Vehicle:Speed",
        ] {
            assert!(!is_valid_path(bad), "{bad:?} should be invalid");
        }
    }

    #[test]
    fn values_match_types() {
        assert!(DataValue::Bool(true).matches_type(DataType::Bool));
        assert!(DataValue::Float(1.0).matches_type(DataType::Float));
        assert!(DataValue::NotAvailable.matches_type(DataType::Int32));
        assert!(!DataValue::Int32(1).matches_type(DataType::Float));
        assert!(!DataValue::String("x".into()).matches_type(DataType::Bool));
    }

    #[test]
    fn ids_are_handed_out_in_order() {
        let mut db = Database::new();
        let speed = db
            .add(NewEntry::new(
                "Vehicle.Speed",
                DataType::Float,
                EntryType::Sensor,
            ))
            .expect("valid entry");
        let door = db
            .add(NewEntry::new(
                "Vehicle.Cabin.Door.IsOpen",
                DataType::Bool,
                EntryType::Actuator,
            ))
            .expect("valid entry");
        assert_eq!((speed, door), (0, 1));
        assert_eq!(db.len(), 2);
        assert_eq!(db.id_by_path("Vehicle.Speed"), Some(0));
    }

    #[test]
    fn registering_twice_returns_the_same_id() {
        let mut db = Database::new();
        let first = db
            .add(NewEntry::new(
                "Vehicle.Speed",
                DataType::Float,
                EntryType::Sensor,
            ))
            .unwrap();
        let second = db
            .add(NewEntry::new(
                "Vehicle.Speed",
                DataType::Int32,
                EntryType::Sensor,
            ))
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(db.len(), 1);
        // The original registration wins.
        let entry = db.get_entry_by_id(first).unwrap();
        assert_eq!(entry.metadata.data_type, DataType::Float);
    }

    #[test]
    fn new_entries_start_not_available() {
        let mut db = Database::new();
        let id = db
            .add(NewEntry::new(
                "Vehicle.Speed",
                DataType::Float,
                EntryType::Sensor,
            ))
            .unwrap();
        let entry = db.get_entry_by_path("Vehicle.Speed").unwrap();
        assert_eq!(entry.metadata.id, id);
        assert_eq!(entry.metadata.path, "Vehicle.Speed");
        assert_eq!(entry.datapoint.value, DataValue::NotAvailable);
        assert!(entry.actuator_target.is_none());
    }

    #[test]
    fn rejects_invalid_registrations() {
        let mut db = Database::new();
        assert_eq!(
            db.add(NewEntry::new(
                "Vehicle..Speed",
                DataType::Float,
                EntryType::Sensor
            ))
            .unwrap_err(),
            RegistrationError::ValidationError
        );
        let wrong_allowed = NewEntry::new("Vehicle.Gear", DataType::String, EntryType::Sensor)
            .allowed(vec![DataValue::String("P".into()), DataValue::Int32(1)]);
        assert_eq!(
            db.add(wrong_allowed).unwrap_err(),
            RegistrationError::ValidationError
        );
        assert!(
            db.is_empty(),
            "failed registrations must not store anything"
        );
    }

    #[test]
    fn unknown_lookups_are_not_found() {
        let db = Database::new();
        assert_eq!(db.get_entry_by_id(7).unwrap_err(), ReadError::NotFound);
        assert_eq!(
            db.get_entry_by_path("Vehicle.Nope").unwrap_err(),
            ReadError::NotFound
        );
    }
}
