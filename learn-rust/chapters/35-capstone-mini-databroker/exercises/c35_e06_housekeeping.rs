// c35_e06_housekeeping.rs
//
// CONCEPT: Capstone step 6 — a background task that cleans up, and shutdown.
// GUIDE:   chapters/35-capstone-mini-databroker/guide.md  (section "Step 6: housekeeping and shutdown")
// DATABROKER: databroker/src/broker.rs — `DataBroker::start_housekeeping_task`,
//             `Subscriptions::cleanup_change_subscriptions`, `DataBroker::shutdown`,
//             `get_shutdown_trigger`
// EXPECT:  tests fail: the functions marked `todo!()` panic
//
// When a gRPC client disconnects, tonic drops the response stream. That
// drops our `BroadcastStream`, which drops the broadcast *receiver*. The
// broker never gets a "goodbye" call — instead a housekeeping task wakes up
// every second and removes subscriptions whose `sender.receiver_count()` is
// 0, or whose permissions (JWT) have expired. This is RAII doing
// distributed cleanup: dropping a value *is* the signal.
//
// Steps 1-5 are finished below. By the end of this step you have a complete
// mini-databroker. Go and compare it with databroker/src/broker.rs!
//
// YOUR TASK
//   1. `Subscriptions::cleanup_change_subscriptions` (hint: `HashMap::retain`)
//   2. `DataBroker::start_housekeeping_task` — spawn a task that ticks every
//      `HOUSEKEEPING_INTERVAL` and cleans up, until shutdown is triggered.
//   3. `DataBroker::shutdown` — clear all subscriptions (this drops their
//      senders, so every subscriber's stream ends), then fire the trigger.
//   Do not edit the tests.

// I AM NOT DONE

use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

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
// STEP 3 (finished): permissions and authorized views
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

// ===========================================================================
// STEP 4 (finished): the async broker, subscriptions and notify
// ===========================================================================

/// One changed entry, as delivered to a subscriber.
#[derive(Debug, Clone)]
pub struct ChangeNotification {
    pub id: i32,
    pub path: String,
    pub datapoint: Option<Datapoint>,
    pub actuator_target: Option<Option<Datapoint>>,
    /// Which of the fields above are filled in.
    pub fields: HashSet<Field>,
}

/// One message on a subscription channel.
#[derive(Debug, Clone, Default)]
pub struct EntryUpdates {
    pub updates: Vec<ChangeNotification>,
}

#[derive(Debug, PartialEq)]
pub struct NotificationError;

pub struct ChangeSubscription {
    /// id -> the fields this subscriber wants to hear about
    entries: HashMap<i32, HashSet<Field>>,
    /// `Option` because the real broker sends `None` to mean "something went
    /// wrong / you lagged" — see step 5.
    sender: broadcast::Sender<Option<EntryUpdates>>,
    permissions: Permissions,
}

impl ChangeSubscription {
    pub fn new(
        entries: HashMap<i32, HashSet<Field>>,
        sender: broadcast::Sender<Option<EntryUpdates>>,
        permissions: Permissions,
    ) -> Self {
        ChangeSubscription {
            entries,
            sender,
            permissions,
        }
    }

    /// Builds the notification for `entry`, filling in only `fields`.
    fn build_notification(entry: &Entry, fields: HashSet<Field>) -> ChangeNotification {
        ChangeNotification {
            id: entry.metadata.id,
            path: entry.metadata.path.clone(),
            datapoint: fields
                .contains(&Field::Datapoint)
                .then(|| entry.datapoint.clone()),
            actuator_target: fields
                .contains(&Field::ActuatorTarget)
                .then(|| entry.actuator_target.clone()),
            fields,
        }
    }

    /// Sends this subscriber what it needs to know. Read the database through
    /// `db.authorized_read_access(&self.permissions)`.
    ///
    /// `changed == None` — the *initial* notification: one
    ///   `ChangeNotification` for every subscribed entry that is readable,
    ///   with all subscribed fields. Always send it (even if empty).
    ///
    /// `changed == Some(map)` — after an update: for every changed id that
    ///   this subscriber subscribed to, keep only the fields that are both
    ///   subscribed *and* changed (set intersection). Skip ids with no
    ///   overlap and entries that can't be read — but if a read fails with
    ///   `PermissionExpired`, stop and return `Err(NotificationError)`.
    ///   If nothing is left, send nothing and return `Ok(())`.
    ///
    /// Sort `updates` by id so the output is deterministic. If
    /// `sender.send(..)` fails (no receivers left) return `Err(NotificationError)`.
    pub fn notify(
        &self,
        changed: Option<&HashMap<i32, HashSet<Field>>>,
        db: &Database,
    ) -> Result<(), NotificationError> {
        let db = db.authorized_read_access(&self.permissions);
        let mut notifications = EntryUpdates::default();
        match changed {
            None => {
                for (id, fields) in &self.entries {
                    if let Ok(entry) = db.get_entry_by_id(*id) {
                        notifications
                            .updates
                            .push(Self::build_notification(entry, fields.clone()));
                    }
                }
            }
            Some(changed) => {
                for (id, changed_fields) in changed {
                    let Some(subscribed) = self.entries.get(id) else {
                        continue;
                    };
                    let relevant: HashSet<Field> =
                        subscribed.intersection(changed_fields).copied().collect();
                    if relevant.is_empty() {
                        continue;
                    }
                    match db.get_entry_by_id(*id) {
                        Ok(entry) => notifications
                            .updates
                            .push(Self::build_notification(entry, relevant)),
                        Err(ReadError::PermissionExpired) => return Err(NotificationError),
                        Err(_) => {}
                    }
                }
                if notifications.updates.is_empty() {
                    return Ok(());
                }
            }
        }
        notifications.updates.sort_by_key(|n| n.id);
        // WHY: `send` only fails when every receiver has been dropped.
        self.sender
            .send(Some(notifications))
            .map(|_receiver_count| ())
            .map_err(|_| NotificationError)
    }
}

#[derive(Default)]
pub struct Subscriptions {
    change_subscriptions: HashMap<Uuid, ChangeSubscription>,
}

impl Subscriptions {
    pub fn add_change_subscription(&mut self, subscription: ChangeSubscription) -> Uuid {
        let uuid = Uuid::new_v4();
        self.change_subscriptions.insert(uuid, subscription);
        uuid
    }

    pub fn len(&self) -> usize {
        self.change_subscriptions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.change_subscriptions.is_empty()
    }

    /// Notifies every subscription. One failing subscriber must not stop the
    /// others, so we remember the error and keep going.
    pub fn notify(
        &self,
        changed: Option<&HashMap<i32, HashSet<Field>>>,
        db: &Database,
    ) -> Result<(), NotificationError> {
        let mut result = Ok(());
        for subscription in self.change_subscriptions.values() {
            if let Err(err) = subscription.notify(changed, db) {
                result = Err(err);
            }
        }
        result
    }
}

/// Cheap to clone: every field is an `Arc` or a channel handle.
#[derive(Clone)]
pub struct DataBroker {
    database: Arc<RwLock<Database>>,
    subscriptions: Arc<RwLock<Subscriptions>>,
    /// Used in step 6 to stop background tasks.
    shutdown_trigger: broadcast::Sender<()>,
}

impl DataBroker {
    pub fn new() -> Self {
        let (shutdown_trigger, _) = broadcast::channel(1);
        DataBroker {
            database: Default::default(),
            subscriptions: Default::default(),
            shutdown_trigger,
        }
    }

    pub fn authorized_access<'b, 'p>(
        &'b self,
        permissions: &'p Permissions,
    ) -> AuthorizedAccess<'b, 'p> {
        AuthorizedAccess {
            broker: self,
            permissions,
        }
    }
}

impl Default for DataBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// The async, permission-scoped front door (every gRPC handler starts with
/// `broker.authorized_access(&permissions)`).
pub struct AuthorizedAccess<'b, 'p> {
    broker: &'b DataBroker,
    permissions: &'p Permissions,
}

impl AuthorizedAccess<'_, '_> {
    pub async fn add_entry(&self, new: NewEntry) -> Result<i32, RegistrationError> {
        self.broker
            .database
            .write()
            .await
            .authorized_write_access(self.permissions)
            .add(new)
    }

    pub async fn id_by_path(&self, path: &str) -> Option<i32> {
        self.broker.database.read().await.id_by_path(path)
    }

    /// Returns an owned *clone*: a reference could not escape the read guard,
    /// which is released at the end of this statement.
    pub async fn get_datapoint(&self, id: i32) -> Result<Datapoint, ReadError> {
        self.broker
            .database
            .read()
            .await
            .authorized_read_access(self.permissions)
            .get_entry_by_id(id)
            .map(|entry| entry.datapoint.clone())
    }

    /// Applies all updates under one write lock. Failed updates are collected
    /// as `(id, error)`; the others still apply. Then notify subscribers
    /// about the ids that changed (ignore notification errors — step 6's
    /// housekeeping removes dead subscriptions).
    ///
    /// Lock choreography (same as the real broker):
    ///   1. `database.write().await`
    ///   2. apply through `authorized_write_access(self.permissions)`
    ///   3. `downgrade()` the write guard to a read guard — other readers may
    ///      proceed, but no writer can sneak in before subscribers are notified
    ///   4. `subscriptions.read().await.notify(Some(&changed), &db)`
    pub async fn update_entries(
        &self,
        updates: impl IntoIterator<Item = (i32, EntryUpdate)>,
    ) -> Result<(), Vec<(i32, UpdateError)>> {
        let mut errors = Vec::new();
        let mut changed: HashMap<i32, HashSet<Field>> = HashMap::new();

        let mut db = self.broker.database.write().await;
        {
            let mut db_write = db.authorized_write_access(self.permissions);
            for (id, update) in updates {
                match db_write.update(id, update) {
                    Ok(fields) if !fields.is_empty() => {
                        changed.insert(id, fields);
                    }
                    Ok(_) => {}
                    Err(err) => errors.push((id, err)),
                }
            }
        } // WHY: `db_write` borrows `db` mutably; it must end before downgrade.

        let db = db.downgrade();
        if !changed.is_empty() {
            // Dead subscribers are removed by housekeeping (step 6).
            let _ = self
                .broker
                .subscriptions
                .read()
                .await
                .notify(Some(&changed), &db);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ===========================================================================
// STEP 5 (finished): subscribe as a Stream
// ===========================================================================

pub const MAX_SUBSCRIBE_BUFFER_SIZE: usize = 1000;

#[derive(Debug, PartialEq)]
pub enum SubscriptionError {
    NotFound,
    PermissionDenied,
    InvalidInput,
    InvalidBufferSize,
}

/// The real `subscribe` returns `impl Stream<Item = Option<EntryUpdates>>`;
/// the gRPC layer then boxes it into exactly this shape
/// (`type SubscribeStream = Pin<Box<dyn Stream<..> + Send + ..>>` in val.rs).
/// Boxing here keeps the signature simple and makes the stream `Unpin`, so
/// callers can use `.next().await` directly.
pub type UpdateStream = Pin<Box<dyn Stream<Item = Option<EntryUpdates>> + Send>>;

impl AuthorizedAccess<'_, '_> {
    /// Rules (the same as the real broker):
    ///   * empty `valid_entries`                  -> `InvalidInput`
    ///   * `buffer_size > MAX_SUBSCRIBE_BUFFER_SIZE` -> `InvalidBufferSize`
    ///   * channel capacity = `buffer_size + 1` ("n old messages plus the
    ///     latest"), or `1` when `buffer_size` is `None`
    ///   * create the channel and a `ChangeSubscription` (with a clone of
    ///     `self.permissions`), send the initial notification
    ///     (`notify(None, ..)`), register it in `subscriptions`
    ///   * return the receiver as a stream: `Ok(msg)` -> `msg`,
    ///     `Err(Lagged(_))` -> `None`
    ///
    /// Bonus: the real broker releases the database lock between the initial
    /// notification and registering the subscription. What can a subscriber
    /// miss because of that? Can you avoid it? (see the solution / guide)
    pub async fn subscribe(
        &self,
        valid_entries: HashMap<i32, HashSet<Field>>,
        buffer_size: Option<usize>,
    ) -> Result<UpdateStream, SubscriptionError> {
        if valid_entries.is_empty() {
            return Err(SubscriptionError::InvalidInput);
        }
        let capacity = match buffer_size {
            Some(size) if size > MAX_SUBSCRIBE_BUFFER_SIZE => {
                return Err(SubscriptionError::InvalidBufferSize)
            }
            Some(size) => size + 1,
            None => 1,
        };

        let (sender, receiver) = broadcast::channel(capacity);
        let subscription = ChangeSubscription::new(valid_entries, sender, self.permissions.clone());

        // WHY: we keep the database read lock until the subscription is
        // registered. Writers need the write lock, so no update can slip in
        // between "initial snapshot" and "now listening" — the real broker
        // drops the lock in between and can miss such an update.
        // Lock order is database -> subscriptions, same as `update_entries`,
        // so this cannot deadlock with it.
        let db = self.broker.database.read().await;
        let _ = subscription.notify(None, &db);
        self.broker
            .subscriptions
            .write()
            .await
            .add_change_subscription(subscription);
        drop(db);

        let stream = BroadcastStream::new(receiver).map(|result| match result {
            Ok(message) => message,
            Err(BroadcastStreamRecvError::Lagged(_missed)) => None,
        });
        Ok(Box::pin(stream))
    }

    /// Subscribes to the *datapoints* of `paths`. Resolve each path through
    /// an authorized read view: `NotFound` -> `SubscriptionError::NotFound`,
    /// any other read error -> `SubscriptionError::PermissionDenied`.
    ///
    /// Careful: release the database guard *before* calling `subscribe`,
    /// which takes the lock again (tokio's RwLock is fair: a re-entrant read
    /// while a writer is queued would deadlock).
    pub async fn subscribe_paths(
        &self,
        paths: &[&str],
        buffer_size: Option<usize>,
    ) -> Result<UpdateStream, SubscriptionError> {
        let entries = {
            let db = self.broker.database.read().await;
            let view = db.authorized_read_access(self.permissions);
            let mut entries = HashMap::with_capacity(paths.len());
            for path in paths {
                let entry = view.get_entry_by_path(path).map_err(|err| match err {
                    ReadError::NotFound => SubscriptionError::NotFound,
                    ReadError::PermissionDenied | ReadError::PermissionExpired => {
                        SubscriptionError::PermissionDenied
                    }
                })?;
                entries.insert(entry.metadata.id, HashSet::from([Field::Datapoint]));
            }
            entries
        }; // WHY: guard dropped here, before `subscribe` locks again.
        self.subscribe(entries, buffer_size).await
    }
}

// ===========================================================================
// STEP 6 (this step): housekeeping and shutdown
// ===========================================================================

pub const HOUSEKEEPING_INTERVAL: Duration = Duration::from_secs(1);

impl Subscriptions {
    /// Removes every change subscription that nobody listens to any more
    /// (`sender.receiver_count() == 0`) or whose permissions have expired.
    /// Returns the uuids that were removed.
    pub fn cleanup_change_subscriptions(&mut self) -> Vec<Uuid> {
        todo!()
    }

    pub fn clear(&mut self) {
        self.change_subscriptions.clear();
    }
}

impl DataBroker {
    /// Spawns the housekeeping loop and returns its `JoinHandle`.
    ///
    ///   * clone what the task needs (`Arc::clone(&self.subscriptions)`);
    ///     the task must be `'static`, so it can't borrow `self`
    ///   * subscribe to the shutdown trigger *before* spawning (why? see the
    ///     `shutdown_right_after_start_still_stops_the_task` test)
    ///   * `tokio::time::interval(HOUSEKEEPING_INTERVAL)` +
    ///     `tokio::select!` between `interval.tick()` and the shutdown receiver
    ///   * on each tick: `cleanup_change_subscriptions()` under a write lock
    ///     (keep the guard's scope tiny)
    ///
    /// (The real task has no shutdown branch; it simply dies with the runtime.)
    pub fn start_housekeeping_task(&self) -> JoinHandle<()> {
        todo!()
    }

    /// Clears all subscriptions, then sends `()` on `shutdown_trigger`
    /// (ignore the error you get when nobody is listening).
    pub async fn shutdown(&self) {
        todo!()
    }

    pub fn shutdown_trigger(&self) -> broadcast::Receiver<()> {
        self.shutdown_trigger.subscribe()
    }

    pub async fn subscription_count(&self) -> usize {
        self.subscriptions.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (DataBroker, i32) {
        let broker = DataBroker::new();
        let all = Permissions::allow_all();
        let speed = broker
            .authorized_access(&all)
            .add_entry(NewEntry::new(
                "Vehicle.Speed",
                DataType::Int32,
                EntryType::Sensor,
            ))
            .await
            .unwrap();
        (broker, speed)
    }

    async fn subscribe(broker: &DataBroker, permissions: &Permissions, id: i32) -> UpdateStream {
        broker
            .authorized_access(permissions)
            .subscribe(
                HashMap::from([(id, HashSet::from([Field::Datapoint]))]),
                None,
            )
            .await
            .unwrap()
    }

    fn expired() -> Permissions {
        Permissions {
            expires_at: Some(SystemTime::now() - Duration::from_secs(1)),
            ..Permissions::allow_all()
        }
    }

    #[tokio::test]
    async fn cleanup_removes_dropped_and_expired() {
        let (broker, speed) = setup().await;
        let all = Permissions::allow_all();
        let _alive = subscribe(&broker, &all, speed).await;
        let dropped = subscribe(&broker, &all, speed).await;
        drop(dropped);
        let _expired = subscribe(&broker, &expired(), speed).await;
        assert_eq!(broker.subscription_count().await, 3);

        let removed = broker
            .subscriptions
            .write()
            .await
            .cleanup_change_subscriptions();
        assert_eq!(removed.len(), 2);
        assert_eq!(broker.subscription_count().await, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn housekeeping_runs_every_interval() {
        let (broker, speed) = setup().await;
        let all = Permissions::allow_all();
        let task = broker.start_housekeeping_task();

        let _alive = subscribe(&broker, &all, speed).await;
        let gone = subscribe(&broker, &all, speed).await;
        drop(gone);
        assert_eq!(broker.subscription_count().await, 2);

        // With a paused clock, sleeping just fast-forwards virtual time.
        tokio::time::sleep(HOUSEKEEPING_INTERVAL + Duration::from_millis(100)).await;
        assert_eq!(broker.subscription_count().await, 1);

        broker.shutdown().await;
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("housekeeping must stop on shutdown")
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_ends_every_subscriber_stream() {
        let (broker, speed) = setup().await;
        let all = Permissions::allow_all();
        let mut stream = subscribe(&broker, &all, speed).await;
        let mut trigger = broker.shutdown_trigger();

        broker.shutdown().await;
        trigger.recv().await.expect("trigger fired");

        // The initial notification is still buffered, then the stream ends
        // because the sender was dropped by `clear()`.
        assert!(stream.next().await.is_some());
        let end = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("stream must end, not hang");
        assert!(end.is_none());
        assert_eq!(broker.subscription_count().await, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_right_after_start_still_stops_the_task() {
        let (broker, _) = setup().await;
        let task = broker.start_housekeeping_task();
        // No `.await` between start and shutdown: the task hasn't run yet.
        broker.shutdown().await;
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("task missed the shutdown signal")
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn full_round_trip() {
        let (broker, speed) = setup().await;
        let task = broker.start_housekeeping_task();
        let all = Permissions::allow_all();
        let mut stream = broker
            .authorized_access(&all)
            .subscribe_paths(&["Vehicle.Speed"], Some(4))
            .await
            .unwrap();
        stream.next().await.expect("initial");

        let publisher = {
            let broker = broker.clone();
            tokio::spawn(async move {
                let all = Permissions::allow_all();
                for v in 1..=3 {
                    let update = EntryUpdate {
                        datapoint: Some(Datapoint::now(DataValue::Int32(v))),
                        ..Default::default()
                    };
                    broker
                        .authorized_access(&all)
                        .update_entries([(speed, update)])
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
        };

        let mut seen = Vec::new();
        while seen.len() < 3 {
            let msg = stream.next().await.unwrap().expect("no lag");
            seen.push(msg.updates[0].datapoint.clone().unwrap().value);
        }
        assert_eq!(
            seen,
            vec![
                DataValue::Int32(1),
                DataValue::Int32(2),
                DataValue::Int32(3)
            ]
        );
        publisher.await.unwrap();

        drop(stream);
        tokio::time::sleep(HOUSEKEEPING_INTERVAL * 2).await;
        assert_eq!(broker.subscription_count().await, 0);
        broker.shutdown().await;
        task.await.unwrap();
    }
}
