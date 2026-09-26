// c35_e03_authorized_access.rs
//
// CONCEPT: Capstone step 3 — permission-checked views with lifetimes.
// GUIDE:   chapters/35-capstone-mini-databroker/guide.md  (section "Step 3: authorized access")
// DATABROKER: databroker/src/broker.rs — `DatabaseReadAccess<'a, 'b>`,
//             `DatabaseWriteAccess<'a, 'b>`, `Database::authorized_read_access`;
//             databroker/src/permissions.rs — `Permissions::can_read` & friends
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// In the databroker you never touch the `Database` directly. You ask for a
// *view* bound to a set of permissions (decoded from the caller's JWT):
//
//     db.authorized_read_access(&permissions).get_entry_by_id(id)
//
// The view borrows both the database and the permissions, so the compiler
// guarantees it can't outlive either — and every read/write through it is
// checked. Forgetting a permission check becomes very hard.
//
// Steps 1-2 are finished below. New here: `Permissions` (given, a
// simplified copy of permissions.rs) and the two view types.
//
// YOUR TASK
//   1. The three `From<PermissionError>` impls (so `?` converts errors).
//   2. `DatabaseReadAccess::{get_entry_by_id, get_entry_by_path, readable_paths}`.
//   3. `DatabaseWriteAccess::{add, update}`.
//   Note the return type `&'db Entry`: the reference borrows the *database*,
//   not the view, so it stays usable after the view is dropped (see the
//   `entries_outlive_the_view` test). Do not edit the tests.

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
// STEP 2 (finished): updates and validation
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

// ===========================================================================
// STEP 3 (this step): permissions and authorized views
// ===========================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionError {
    Denied,
    Expired,
}

/// VSS glob matching, simplified from `glob::Matcher` in databroker/src/glob.rs:
/// a pattern without `*` matches the path itself *and everything below it*
/// ("Vehicle.Cabin" matches "Vehicle.Cabin.Door.IsOpen").
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let glob = pattern.replace('.', "/");
    let path = path.replace('.', "/");
    if pattern.contains('*') {
        glob_match::glob_match(&glob, &path)
    } else {
        path == glob || path.starts_with(&format!("{glob}/"))
    }
}

/// What a caller may do, as a list of VSS globs per action.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub expires_at: Option<SystemTime>,
    pub read: Vec<String>,
    pub actuate: Vec<String>,
    pub provide: Vec<String>,
    pub create: Vec<String>,
}

impl Permissions {
    pub fn allow_all() -> Self {
        let all = vec!["**".to_string()];
        Permissions {
            expires_at: None,
            read: all.clone(),
            actuate: all.clone(),
            provide: all.clone(),
            create: all,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at < SystemTime::now())
    }

    fn check(&self, globs: &[&[String]], path: &str) -> Result<(), PermissionError> {
        if self.is_expired() {
            return Err(PermissionError::Expired);
        }
        let allowed = globs
            .iter()
            .flat_map(|list| list.iter())
            .any(|glob| glob_matches(glob, path));
        if allowed {
            Ok(())
        } else {
            Err(PermissionError::Denied)
        }
    }

    /// Reading is implied by every other permission (same convention as the
    /// real `Permissions::can_read`).
    pub fn can_read(&self, path: &str) -> Result<(), PermissionError> {
        self.check(
            &[&self.read, &self.actuate, &self.provide, &self.create],
            path,
        )
    }

    pub fn can_write_datapoint(&self, path: &str) -> Result<(), PermissionError> {
        self.check(&[&self.provide], path)
    }

    pub fn can_write_actuator_target(&self, path: &str) -> Result<(), PermissionError> {
        self.check(&[&self.actuate], path)
    }

    pub fn can_create(&self, path: &str) -> Result<(), PermissionError> {
        self.check(&[&self.create], path)
    }
}

impl From<PermissionError> for ReadError {
    fn from(err: PermissionError) -> Self {
        match err {
            PermissionError::Denied => ReadError::PermissionDenied,
            PermissionError::Expired => ReadError::PermissionExpired,
        }
    }
}

impl From<PermissionError> for UpdateError {
    fn from(err: PermissionError) -> Self {
        match err {
            PermissionError::Denied => UpdateError::PermissionDenied,
            PermissionError::Expired => UpdateError::PermissionExpired,
        }
    }
}

impl From<PermissionError> for RegistrationError {
    fn from(err: PermissionError) -> Self {
        match err {
            PermissionError::Denied => RegistrationError::PermissionDenied,
            PermissionError::Expired => RegistrationError::PermissionExpired,
        }
    }
}

/// A read-only view. `'db` is how long we may borrow the database,
/// `'perm` how long we may borrow the permissions. (The real code calls
/// them `'a` and `'b`.)
pub struct DatabaseReadAccess<'db, 'perm> {
    db: &'db Database,
    permissions: &'perm Permissions,
}

/// A read-write view. Holding `&mut Database` means no other view can exist
/// at the same time — the borrow checker enforces "one writer XOR many readers".
pub struct DatabaseWriteAccess<'db, 'perm> {
    db: &'db mut Database,
    permissions: &'perm Permissions,
}

impl Database {
    pub fn authorized_read_access<'db, 'perm>(
        &'db self,
        permissions: &'perm Permissions,
    ) -> DatabaseReadAccess<'db, 'perm> {
        DatabaseReadAccess {
            db: self,
            permissions,
        }
    }

    pub fn authorized_write_access<'db, 'perm>(
        &'db mut self,
        permissions: &'perm Permissions,
    ) -> DatabaseWriteAccess<'db, 'perm> {
        DatabaseWriteAccess {
            db: self,
            permissions,
        }
    }
}

impl<'db> DatabaseReadAccess<'db, '_> {
    /// Unknown id -> `NotFound`; otherwise the entry, if `can_read` allows it.
    pub fn get_entry_by_id(&self, id: i32) -> Result<&'db Entry, ReadError> {
        let entry = self.db.entries.get(&id).ok_or(ReadError::NotFound)?;
        // WHY: `?` calls `From<PermissionError> for ReadError` for us.
        self.permissions.can_read(&entry.metadata.path)?;
        Ok(entry)
    }

    pub fn get_entry_by_path(&self, path: &str) -> Result<&'db Entry, ReadError> {
        let id = self.db.id_by_path(path).ok_or(ReadError::NotFound)?;
        self.get_entry_by_id(id)
    }

    /// All paths this caller may read, sorted alphabetically.
    pub fn readable_paths(&self) -> Vec<&'db str> {
        let mut paths: Vec<&'db str> = self
            .db
            .entries
            .values()
            .map(|entry| entry.metadata.path.as_str())
            .filter(|path| self.permissions.can_read(path).is_ok())
            .collect();
        paths.sort_unstable();
        paths
    }
}

impl DatabaseWriteAccess<'_, '_> {
    /// Needs `can_create` on the path, then behaves like `Database::add`.
    pub fn add(&mut self, new: NewEntry) -> Result<i32, RegistrationError> {
        self.permissions.can_create(&new.path)?;
        self.db.add(new)
    }

    /// Unknown id -> `NotFound`. A new datapoint needs `can_write_datapoint`
    /// (the *provide* permission); a new actuator target needs
    /// `can_write_actuator_target` (*actuate*). Then `Database::update`.
    pub fn update(&mut self, id: i32, update: EntryUpdate) -> Result<HashSet<Field>, UpdateError> {
        let path = &self
            .db
            .get_entry_by_id(id)
            .map_err(|_| UpdateError::NotFound)?
            .metadata
            .path;
        if update.datapoint.is_some() {
            self.permissions.can_write_datapoint(path)?;
        }
        if update.actuator_target.is_some() {
            self.permissions.can_write_actuator_target(path)?;
        }
        // WHY: the shared borrow `path` ends above, so taking `&mut` here is
        // fine (non-lexical lifetimes).
        self.db.update(id, update)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn setup() -> (Database, i32, i32) {
        let mut db = Database::new();
        let speed = db
            .add(NewEntry::new(
                "Vehicle.Speed",
                DataType::Float,
                EntryType::Sensor,
            ))
            .unwrap();
        let door = db
            .add(NewEntry::new(
                "Vehicle.Cabin.Door.IsOpen",
                DataType::Bool,
                EntryType::Actuator,
            ))
            .unwrap();
        (db, speed, door)
    }

    fn cabin_reader() -> Permissions {
        Permissions {
            read: vec!["Vehicle.Cabin".into()],
            ..Default::default()
        }
    }

    fn value(v: DataValue) -> EntryUpdate {
        EntryUpdate {
            datapoint: Some(Datapoint::now(v)),
            ..Default::default()
        }
    }

    #[test]
    fn errors_convert() {
        assert_eq!(
            ReadError::from(PermissionError::Denied),
            ReadError::PermissionDenied
        );
        assert_eq!(
            UpdateError::from(PermissionError::Expired),
            UpdateError::PermissionExpired
        );
        assert_eq!(
            RegistrationError::from(PermissionError::Denied),
            RegistrationError::PermissionDenied
        );
    }

    #[test]
    fn read_is_checked() {
        let (db, speed, door) = setup();
        let perms = cabin_reader();
        let view = db.authorized_read_access(&perms);
        assert!(view.get_entry_by_id(door).is_ok());
        assert_eq!(
            view.get_entry_by_id(speed).unwrap_err(),
            ReadError::PermissionDenied
        );
        assert_eq!(
            view.get_entry_by_path("Vehicle.Speed").unwrap_err(),
            ReadError::PermissionDenied
        );
        assert_eq!(
            view.get_entry_by_path("Vehicle.Nope").unwrap_err(),
            ReadError::NotFound
        );
        assert_eq!(view.get_entry_by_id(42).unwrap_err(), ReadError::NotFound);
    }

    #[test]
    fn expired_permissions_fail_with_expired() {
        let (db, speed, _) = setup();
        let perms = Permissions {
            expires_at: Some(SystemTime::now() - Duration::from_secs(1)),
            ..Permissions::allow_all()
        };
        assert_eq!(
            db.authorized_read_access(&perms)
                .get_entry_by_id(speed)
                .unwrap_err(),
            ReadError::PermissionExpired
        );
    }

    #[test]
    fn readable_paths_are_filtered_and_sorted() {
        let (mut db, _, _) = setup();
        db.add(NewEntry::new(
            "Vehicle.Cabin.Light.IsOn",
            DataType::Bool,
            EntryType::Actuator,
        ))
        .unwrap();
        let perms = cabin_reader();
        assert_eq!(
            db.authorized_read_access(&perms).readable_paths(),
            vec!["Vehicle.Cabin.Door.IsOpen", "Vehicle.Cabin.Light.IsOn"]
        );
        let all = Permissions::allow_all();
        assert_eq!(db.authorized_read_access(&all).readable_paths().len(), 3);
    }

    #[test]
    fn entries_outlive_the_view() {
        let (db, speed, _) = setup();
        let perms = Permissions::allow_all();
        let entry = {
            let view = db.authorized_read_access(&perms);
            view.get_entry_by_id(speed).unwrap()
        }; // <- the view is dropped here, `entry` still borrows `db`
        assert_eq!(entry.metadata.path, "Vehicle.Speed");
    }

    #[test]
    fn provide_and_actuate_are_separate() {
        let (mut db, speed, door) = setup();
        let provider = Permissions {
            provide: vec!["Vehicle.Speed".into()],
            ..Default::default()
        };
        let mut view = db.authorized_write_access(&provider);
        assert!(view.update(speed, value(DataValue::Float(3.0))).is_ok());
        assert_eq!(
            view.update(door, value(DataValue::Bool(true))).unwrap_err(),
            UpdateError::PermissionDenied
        );
        let target = EntryUpdate {
            actuator_target: Some(Some(Datapoint::now(DataValue::Bool(true)))),
            ..Default::default()
        };
        assert_eq!(
            view.update(door, target.clone()).unwrap_err(),
            UpdateError::PermissionDenied
        );

        let app = Permissions {
            actuate: vec!["Vehicle.Cabin.*.*".into()],
            ..Default::default()
        };
        let mut view = db.authorized_write_access(&app);
        assert!(view.update(door, target).is_ok());
        assert_eq!(
            view.update(77, EntryUpdate::default()).unwrap_err(),
            UpdateError::NotFound
        );
    }

    #[test]
    fn create_needs_permission() {
        let mut db = Database::new();
        let reader = Permissions::allow_all();
        let nobody = Permissions {
            read: reader.read.clone(),
            ..Default::default()
        };
        assert_eq!(
            db.authorized_write_access(&nobody)
                .add(NewEntry::new(
                    "Vehicle.X",
                    DataType::Bool,
                    EntryType::Sensor
                ))
                .unwrap_err(),
            RegistrationError::PermissionDenied
        );
        assert_eq!(
            db.authorized_write_access(&reader)
                .add(NewEntry::new(
                    "Vehicle.X",
                    DataType::Bool,
                    EntryType::Sensor
                ))
                .unwrap(),
            0
        );
    }
}
