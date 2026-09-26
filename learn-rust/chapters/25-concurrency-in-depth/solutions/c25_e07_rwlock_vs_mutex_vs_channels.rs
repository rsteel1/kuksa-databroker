// c25_e07_rwlock_vs_mutex_vs_channels.rs
//
// CONCEPT: one problem, three designs — `Mutex`, `RwLock`, and an actor thread
//          that owns the state and receives commands over a channel — and
//          when each wins.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Choosing: Mutex, RwLock or channels")
// DATABROKER: databroker/src/broker.rs — `DataBroker` (line 179) keeps its
//             `Database` in an `Arc<RwLock<..>>` (many readers, few writers),
//             while each provider is talked to through channels
//             (grpc/kuksa_val_v2/val.rs, `Provider`, line 59): shared state
//             where reads dominate, message passing where a peer owns the data.
// EXPECT:  tests fail: `todo!()` panics
//
// All three stores implement the same `SignalStore` trait, so the same tests
// run against each:
//   * `MutexStore`  - simplest; readers block each other.
//   * `RwLockStore` - many readers at once; writers exclusive.
//   * `ActorStore`  - one thread owns the HashMap; callers send `Command`s
//                     over `std::sync::mpsc` and wait for the reply on a
//                     per-request channel (the request/reply pattern).
//
// YOUR TASK
//   Implement the three stores. For `ActorStore::new`, spawn the owner thread
//   and keep the `Sender`; the thread must exit when every `Sender` is
//   dropped (the loop `for cmd in rx` ends). `Drop for ActorStore` is given.
//   Do not edit the tests.

use std::collections::HashMap;
use std::sync::{mpsc, Mutex, RwLock};
use std::thread::{self, JoinHandle};

trait SignalStore: Send + Sync {
    fn set(&self, path: &str, value: i64);
    fn get(&self, path: &str) -> Option<i64>;
    /// Adds `delta` atomically (no lost updates) and returns the new value.
    fn add(&self, path: &str, delta: i64) -> i64;
}

// ------------------------------------------------------------------ Mutex

#[derive(Default)]
struct MutexStore {
    map: Mutex<HashMap<String, i64>>,
}

impl SignalStore for MutexStore {
    fn set(&self, path: &str, value: i64) {
        self.map
            .lock()
            .expect("poisoned")
            .insert(path.to_string(), value);
    }
    fn get(&self, path: &str) -> Option<i64> {
        self.map.lock().expect("poisoned").get(path).copied()
    }
    fn add(&self, path: &str, delta: i64) -> i64 {
        let mut map = self.map.lock().expect("poisoned");
        let v = map.entry(path.to_string()).or_insert(0);
        *v += delta;
        *v
    }
}

// ----------------------------------------------------------------- RwLock

#[derive(Default)]
struct RwLockStore {
    map: RwLock<HashMap<String, i64>>,
}

impl SignalStore for RwLockStore {
    fn set(&self, path: &str, value: i64) {
        self.map
            .write()
            .expect("poisoned")
            .insert(path.to_string(), value);
    }
    fn get(&self, path: &str) -> Option<i64> {
        self.map.read().expect("poisoned").get(path).copied()
    }
    fn add(&self, path: &str, delta: i64) -> i64 {
        // WHY: read-modify-write needs the WRITE lock for the whole operation;
        // reading under `read()` then writing under `write()` loses updates.
        let mut map = self.map.write().expect("poisoned");
        let v = map.entry(path.to_string()).or_insert(0);
        *v += delta;
        *v
    }
}

// ------------------------------------------------------------------ Actor

enum Command {
    Set(String, i64),
    Get(String, mpsc::Sender<Option<i64>>),
    Add(String, i64, mpsc::Sender<i64>),
}

struct ActorStore {
    // `Mutex` because `mpsc::Sender` wasn't `Sync` before Rust 1.72; with a
    // modern compiler `Sender<T>: Sync` and the Mutex could go.
    tx: Mutex<Option<mpsc::Sender<Command>>>,
    owner: Option<JoinHandle<()>>,
}

impl ActorStore {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let owner = thread::spawn(move || {
            // WHY: this thread is the ONLY one touching `map`, so it needs no
            // lock at all. The channel serialises every request.
            let mut map: HashMap<String, i64> = HashMap::new();
            for cmd in rx {
                match cmd {
                    Command::Set(path, value) => {
                        map.insert(path, value);
                    }
                    Command::Get(path, reply) => {
                        let _ = reply.send(map.get(&path).copied());
                    }
                    Command::Add(path, delta, reply) => {
                        let v = map.entry(path).or_insert(0);
                        *v += delta;
                        let _ = reply.send(*v);
                    }
                }
            }
        });
        ActorStore {
            tx: Mutex::new(Some(tx)),
            owner: Some(owner),
        }
    }

    fn send(&self, cmd: Command) {
        self.tx
            .lock()
            .expect("poisoned")
            .as_ref()
            .expect("store is running")
            .send(cmd)
            .expect("actor thread is alive");
    }
}

impl SignalStore for ActorStore {
    fn set(&self, path: &str, value: i64) {
        self.send(Command::Set(path.to_string(), value));
    }
    fn get(&self, path: &str) -> Option<i64> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(Command::Get(path.to_string(), reply_tx));
        reply_rx.recv().expect("actor replied")
    }
    fn add(&self, path: &str, delta: i64) -> i64 {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(Command::Add(path.to_string(), delta, reply_tx));
        reply_rx.recv().expect("actor replied")
    }
}

impl Drop for ActorStore {
    fn drop(&mut self) {
        // Dropping the last Sender ends the actor's `for cmd in rx` loop.
        if let Ok(mut tx) = self.tx.lock() {
            tx.take();
        }
        if let Some(owner) = self.owner.take() {
            let _ = owner.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stores() -> Vec<(&'static str, Box<dyn SignalStore>)> {
        vec![
            ("mutex", Box::new(MutexStore::default())),
            ("rwlock", Box::new(RwLockStore::default())),
            ("actor", Box::new(ActorStore::new())),
        ]
    }

    #[test]
    fn basic_contract() {
        for (name, store) in stores() {
            assert_eq!(store.get("Vehicle.Speed"), None, "{name}");
            store.set("Vehicle.Speed", 50);
            assert_eq!(store.get("Vehicle.Speed"), Some(50), "{name}");
            assert_eq!(store.add("Vehicle.Speed", 5), 55, "{name}");
            assert_eq!(store.add("Vehicle.New", 3), 3, "{name}: add creates at 0");
        }
    }

    #[test]
    fn concurrent_adds_lose_no_updates() {
        for (name, store) in stores() {
            let store = &*store;
            thread::scope(|s| {
                for _ in 0..8 {
                    s.spawn(|| {
                        for _ in 0..500 {
                            store.add("Vehicle.Counter", 1);
                        }
                    });
                }
                for _ in 0..4 {
                    s.spawn(|| {
                        for _ in 0..500 {
                            let v = store.get("Vehicle.Counter").unwrap_or(0);
                            assert!((0..=4000).contains(&v));
                        }
                    });
                }
            });
            assert_eq!(store.get("Vehicle.Counter"), Some(4000), "{name}");
        }
    }

    #[test]
    fn actor_thread_exits_on_drop() {
        let store = ActorStore::new();
        store.set("x", 1);
        drop(store); // would hang here if the actor loop never ended
    }
}
