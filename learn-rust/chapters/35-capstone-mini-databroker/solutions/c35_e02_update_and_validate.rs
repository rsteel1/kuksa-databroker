// c35_e02_update_and_validate.rs
//
// CONCEPT: Capstone step 2 — partial updates, diffing and validation.
// GUIDE:   chapters/35-capstone-mini-databroker/guide.md  (section "Step 2: updates and validation")
// DATABROKER: databroker/src/broker.rs — `EntryUpdate`, `Entry::diff`, `Entry::validate`,
//             `Entry::validate_value`, `Entry::apply`, `DatabaseWriteAccess::update`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// Providers publish values; the broker must refuse anything that does not
// fit the signal's metadata (type, min/max, allowed list), and must tell the
// caller *which fields actually changed* — that set drives subscriber
// notifications in step 4.
//
// Step 1 is finished below. New in this step: `Field`, `UpdateError`,
// `EntryUpdate` and `DataValue::compare` (given), plus five functions for you.
//
// YOUR TASK
//   Implement, in this order (each doc comment has the rules):
//     `Entry::validate_value`, `Entry::validate`, `Entry::diff`,
//     `Entry::apply`, `Database::update`.
//   Do not edit the tests.

use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::SystemTime;

// ===========================================================================
// STEP 1 (finished): types, entries and the Database
// ===========================================================================

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
        // WHY: matching on a tuple lets us list the legal pairs and reject
        // everything else with one `_` arm.
        matches!(
            (self, data_type),
            (DataValue::NotAvailable, _)
                | (DataValue::Bool(_), DataType::Bool)
                | (DataValue::Int32(_), DataType::Int32)
                | (DataValue::Float(_), DataType::Float)
                | (DataValue::String(_), DataType::String)
        )
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
    // WHY: `split` on "" yields one empty segment, so the empty path is
    // rejected by the same rule as "A..B", ".A" and "A.".
    path.split('.').all(|segment| {
        !segment.is_empty()
            && !segment
                .chars()
                .any(|c| c.is_whitespace() || c == ':' || c == '*')
    })
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
        if !is_valid_path(&new.path) {
            return Err(RegistrationError::ValidationError);
        }
        if let Some(&id) = self.path_to_id.get(&new.path) {
            return Ok(id);
        }
        if let Some(allowed) = &new.allowed {
            if !allowed
                .iter()
                .all(|value| value.matches_type(new.data_type))
            {
                return Err(RegistrationError::ValidationError);
            }
        }

        // WHY: `fetch_add` returns the *previous* value, so ids start at 0.
        // (`&mut self` already guarantees exclusive access, so a plain i32
        // would do — the real broker uses an atomic anyway.)
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let entry = Entry {
            datapoint: Datapoint::now(DataValue::NotAvailable),
            actuator_target: None,
            metadata: Metadata {
                id,
                path: new.path.clone(),
                data_type: new.data_type,
                entry_type: new.entry_type,
                change_type: new.change_type,
                description: new.description,
                min: new.min,
                max: new.max,
                allowed: new.allowed,
            },
        };
        self.path_to_id.insert(new.path, id);
        self.entries.insert(id, entry);
        Ok(id)
    }

    pub fn get_entry_by_id(&self, id: i32) -> Result<&Entry, ReadError> {
        self.entries.get(&id).ok_or(ReadError::NotFound)
    }

    pub fn get_entry_by_path(&self, path: &str) -> Result<&Entry, ReadError> {
        let id = self.path_to_id.get(path).ok_or(ReadError::NotFound)?;
        self.get_entry_by_id(*id)
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

// ===========================================================================
// STEP 2 (this step): updates and validation
// ===========================================================================

/// Which part of an entry changed. Subscribers say which fields they care
/// about; `HashSet<Field>` needs `Eq + Hash`, hence the derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Datapoint,
    ActuatorTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateError {
    NotFound,
    WrongType,
    OutOfBoundsAllowed,
    OutOfBoundsMinMax,
    PermissionDenied,
    PermissionExpired,
}

/// A partial update. `None` means "leave this field alone".
#[derive(Debug, Clone, Default)]
pub struct EntryUpdate {
    pub datapoint: Option<Datapoint>,
    /// Tri-state: `None` = unchanged, `Some(None)` = clear the target,
    /// `Some(Some(dp))` = set a new target.
    pub actuator_target: Option<Option<Datapoint>>,
    /// Metadata can't be changed through an update (the real broker rejects
    /// path/type/description changes with `PermissionDenied`).
    pub description: Option<String>,
}

impl DataValue {
    /// Orders two values if they are comparable (numbers with numbers,
    /// strings with strings). Mixed Int32/Float compares as f64.
    pub fn compare(&self, other: &DataValue) -> Option<CmpOrdering> {
        use DataValue::*;
        match (self, other) {
            (Int32(a), Int32(b)) => Some(a.cmp(b)),
            (Float(a), Float(b)) => a.partial_cmp(b),
            (Int32(a), Float(b)) => f64::from(*a).partial_cmp(&f64::from(*b)),
            (Float(a), Int32(b)) => f64::from(*a).partial_cmp(&f64::from(*b)),
            (String(a), String(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

impl Entry {
    /// Rules, checked in this order:
    ///   * `NotAvailable` is always accepted.
    ///   * wrong type                                -> `WrongType`
    ///   * below `min` / above `max`, or not comparable -> `OutOfBoundsMinMax`
    ///     (equal to min or max is fine)
    ///   * `allowed` is set and doesn't contain it   -> `OutOfBoundsAllowed`
    fn validate_value(&self, value: &DataValue) -> Result<(), UpdateError> {
        if *value == DataValue::NotAvailable {
            return Ok(());
        }
        if !value.matches_type(self.metadata.data_type) {
            return Err(UpdateError::WrongType);
        }
        if let Some(min) = &self.metadata.min {
            // WHY: `compare` returns None for incomparable values; treating
            // that as out-of-bounds is the safe default (the real broker's
            // `match ... { Ok(true) => {}, _ => Err(..) }` does the same).
            match value.compare(min) {
                Some(CmpOrdering::Greater | CmpOrdering::Equal) => {}
                _ => return Err(UpdateError::OutOfBoundsMinMax),
            }
        }
        if let Some(max) = &self.metadata.max {
            match value.compare(max) {
                Some(CmpOrdering::Less | CmpOrdering::Equal) => {}
                _ => return Err(UpdateError::OutOfBoundsMinMax),
            }
        }
        if let Some(allowed) = &self.metadata.allowed {
            if !allowed.contains(value) {
                return Err(UpdateError::OutOfBoundsAllowed);
            }
        }
        Ok(())
    }

    /// Validates the new datapoint (if any) and the new actuator target (if
    /// it is `Some(Some(..))` — clearing a target is always fine).
    pub fn validate(&self, update: &EntryUpdate) -> Result<(), UpdateError> {
        if let Some(datapoint) = &update.datapoint {
            self.validate_value(&datapoint.value)?;
        }
        if let Some(Some(target)) = &update.actuator_target {
            self.validate_value(&target.value)?;
        }
        Ok(())
    }

    /// Reduces an update to what actually changes: for signals that are not
    /// `Continuous`, a datapoint whose *value* equals the current value is
    /// dropped (set to `None`). Everything else passes through.
    pub fn diff(&self, mut update: EntryUpdate) -> EntryUpdate {
        if self.metadata.change_type != ChangeType::Continuous {
            if let Some(datapoint) = &update.datapoint {
                if datapoint.value == self.datapoint.value {
                    update.datapoint = None;
                }
            }
        }
        update
    }

    /// Writes the update into the entry and returns which fields changed.
    /// (Consumes the update: its datapoints are *moved* in, not cloned.)
    pub fn apply(&mut self, update: EntryUpdate) -> HashSet<Field> {
        let mut changed = HashSet::new();
        if let Some(datapoint) = update.datapoint {
            self.datapoint = datapoint;
            changed.insert(Field::Datapoint);
        }
        // WHY: `Some(None)` must reach here as "set to None", so we match
        // only the outer Option.
        if let Some(target) = update.actuator_target {
            self.actuator_target = target;
            changed.insert(Field::ActuatorTarget);
        }
        changed
    }
}

impl Database {
    /// Applies `update` to entry `id`:
    ///   * unknown id                      -> `NotFound`
    ///   * `description` is set            -> `PermissionDenied`
    ///   * then `diff`, then `validate` (errors leave the entry untouched),
    ///     then `apply` and return the changed fields.
    pub fn update(&mut self, id: i32, update: EntryUpdate) -> Result<HashSet<Field>, UpdateError> {
        let entry = self.entries.get_mut(&id).ok_or(UpdateError::NotFound)?;
        if update.description.is_some() {
            return Err(UpdateError::PermissionDenied);
        }
        let update = entry.diff(update);
        entry.validate(&update)?;
        Ok(entry.apply(update))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(v: DataValue) -> EntryUpdate {
        EntryUpdate {
            datapoint: Some(Datapoint::now(v)),
            ..Default::default()
        }
    }

    fn setup() -> (Database, i32, i32, i32) {
        let mut db = Database::new();
        let speed = db
            .add(
                NewEntry::new("Vehicle.Speed", DataType::Float, EntryType::Sensor)
                    .min(DataValue::Float(0.0))
                    .max(DataValue::Int32(250)),
            )
            .unwrap();
        let gear = db
            .add(
                NewEntry::new("Vehicle.Gear", DataType::String, EntryType::Actuator).allowed(vec![
                    DataValue::String("P".into()),
                    DataValue::String("D".into()),
                ]),
            )
            .unwrap();
        let ticks = db
            .add(NewEntry::new("Vehicle.Ticks", DataType::Int32, EntryType::Sensor).continuous())
            .unwrap();
        (db, speed, gear, ticks)
    }

    #[test]
    fn accepts_valid_values_and_reports_changed_fields() {
        let (mut db, speed, _, _) = setup();
        let changed = db.update(speed, value(DataValue::Float(88.5))).unwrap();
        assert_eq!(changed, HashSet::from([Field::Datapoint]));
        assert_eq!(
            db.get_entry_by_id(speed).unwrap().datapoint.value,
            DataValue::Float(88.5)
        );
    }

    #[test]
    fn bounds_are_inclusive() {
        let (mut db, speed, _, _) = setup();
        assert!(db.update(speed, value(DataValue::Float(0.0))).is_ok());
        assert!(db.update(speed, value(DataValue::Float(250.0))).is_ok());
    }

    #[test]
    fn rejects_bad_values_and_leaves_entry_untouched() {
        let (mut db, speed, gear, _) = setup();
        db.update(speed, value(DataValue::Float(10.0))).unwrap();

        assert_eq!(
            db.update(speed, value(DataValue::Int32(10))).unwrap_err(),
            UpdateError::WrongType
        );
        assert_eq!(
            db.update(speed, value(DataValue::Float(-1.0))).unwrap_err(),
            UpdateError::OutOfBoundsMinMax
        );
        assert_eq!(
            db.update(speed, value(DataValue::Float(250.5)))
                .unwrap_err(),
            UpdateError::OutOfBoundsMinMax
        );
        assert_eq!(
            db.update(gear, value(DataValue::String("R".into())))
                .unwrap_err(),
            UpdateError::OutOfBoundsAllowed
        );
        assert_eq!(
            db.get_entry_by_id(speed).unwrap().datapoint.value,
            DataValue::Float(10.0)
        );
    }

    #[test]
    fn not_available_is_always_accepted() {
        let (mut db, speed, gear, _) = setup();
        db.update(speed, value(DataValue::Float(1.0))).unwrap();
        assert!(db.update(speed, value(DataValue::NotAvailable)).is_ok());
        assert!(db.update(gear, value(DataValue::NotAvailable)).is_ok());
    }

    #[test]
    fn on_change_signals_ignore_repeated_values() {
        let (mut db, speed, _, _) = setup();
        assert_eq!(
            db.update(speed, value(DataValue::Float(5.0))).unwrap(),
            HashSet::from([Field::Datapoint])
        );
        assert!(db
            .update(speed, value(DataValue::Float(5.0)))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn continuous_signals_always_change() {
        let (mut db, _, _, ticks) = setup();
        db.update(ticks, value(DataValue::Int32(1))).unwrap();
        assert_eq!(
            db.update(ticks, value(DataValue::Int32(1))).unwrap(),
            HashSet::from([Field::Datapoint])
        );
    }

    #[test]
    fn actuator_target_tristate() {
        let (mut db, _, gear, _) = setup();
        let set = EntryUpdate {
            actuator_target: Some(Some(Datapoint::now(DataValue::String("D".into())))),
            ..Default::default()
        };
        assert_eq!(
            db.update(gear, set).unwrap(),
            HashSet::from([Field::ActuatorTarget])
        );
        assert!(db.get_entry_by_id(gear).unwrap().actuator_target.is_some());

        let invalid = EntryUpdate {
            actuator_target: Some(Some(Datapoint::now(DataValue::String("X".into())))),
            ..Default::default()
        };
        assert_eq!(
            db.update(gear, invalid).unwrap_err(),
            UpdateError::OutOfBoundsAllowed
        );

        let clear = EntryUpdate {
            actuator_target: Some(None),
            ..Default::default()
        };
        assert_eq!(
            db.update(gear, clear).unwrap(),
            HashSet::from([Field::ActuatorTarget])
        );
        assert!(db.get_entry_by_id(gear).unwrap().actuator_target.is_none());

        // `None` means "leave it alone": nothing changes.
        assert!(db.update(gear, EntryUpdate::default()).unwrap().is_empty());
    }

    #[test]
    fn metadata_changes_and_unknown_ids_are_rejected() {
        let (mut db, speed, _, _) = setup();
        let rename = EntryUpdate {
            description: Some("fast".into()),
            ..Default::default()
        };
        assert_eq!(
            db.update(speed, rename).unwrap_err(),
            UpdateError::PermissionDenied
        );
        assert_eq!(
            db.update(99, value(DataValue::Float(1.0))).unwrap_err(),
            UpdateError::NotFound
        );
    }
}
