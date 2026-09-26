// c13_e04_send_sync_providers.rs
//
// CONCEPT: the `Send` and `Sync` marker traits; why `Rc`/`RefCell` can't
//          cross threads; putting `Send + Sync + 'static` bounds on trait
//          objects.
// GUIDE:   chapters/13-threads/guide.md  (section "Send and Sync")
// DATABROKER: databroker/src/broker.rs — `pub trait SignalProvider: Send + Sync
//          + 'static` (line 198) and `Arc<dyn ActuationProvider + Send + Sync +
//          'static>` (line 218). Providers are called from whichever Tokio
//          worker thread happens to run the request.
// EXPECT:  compile error E0308 (the tests register `Arc`s, the registry wants
//          `Rc`s). After switching to `Arc`, E0277: `dyn Provider` cannot be
//          sent/shared between threads safely — until you add the bounds.
//
// `Send`: ownership may move to another thread.
// `Sync`: `&T` may be shared between threads (T is Sync iff &T is Send).
// The compiler derives both automatically from a type's fields. `Rc` (non-
// atomic counter) is neither; `RefCell` is Send but not Sync. `Arc<Mutex<T>>`
// is both (if T: Send). A `dyn Trait` is only Send/Sync if you SAY so.
//
// YOUR TASK
//   Make the registry thread-safe:
//     - replace `Rc` with `Arc` and `RefCell` with `Mutex`,
//     - make the trait objects `dyn Provider + Send + Sync` (or put
//       `Send + Sync` on the trait as a supertrait, like the databroker),
//   then implement `poll_all_in_threads`. Do not edit the tests.

use std::sync::{Arc, Mutex};
use std::thread;

// WHY: a supertrait bound means every `dyn Provider` is Send + Sync, just
// like the databroker's `SignalProvider: Send + Sync + 'static`.
trait Provider: Send + Sync {
    fn name(&self) -> String;
    fn poll(&self) -> i64;
}

struct Counter {
    name: String,
    calls: Mutex<i64>,
}

impl Provider for Counter {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn poll(&self) -> i64 {
        let mut calls = self.calls.lock().expect("lock poisoned");
        *calls += 1;
        *calls
    }
}

#[derive(Default)]
struct Registry {
    providers: Vec<Arc<dyn Provider>>,
}

impl Registry {
    fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers.push(provider);
    }

    /// Polls every provider on its own `thread::spawn` thread and returns
    /// "name=value" strings in registration order.
    fn poll_all_in_threads(&self) -> Vec<String> {
        let handles: Vec<_> = self
            .providers
            .iter()
            .map(|provider| {
                let provider = Arc::clone(provider);
                thread::spawn(move || format!("{}={}", provider.name(), provider.poll()))
            })
            .collect();
        // WHY: joining in spawn order keeps results in registration order.
        handles
            .into_iter()
            .map(|h| h.join().expect("provider thread panicked"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(name: &str) -> Counter {
        Counter {
            name: name.into(),
            calls: Default::default(),
        }
    }

    #[test]
    fn polls_in_threads() {
        let mut registry = Registry::default();
        registry.register(std::sync::Arc::new(counter("a")));
        registry.register(std::sync::Arc::new(counter("b")));
        assert_eq!(registry.poll_all_in_threads(), vec!["a=1", "b=1"]);
        assert_eq!(registry.poll_all_in_threads(), vec!["a=2", "b=2"]);
    }

    #[test]
    fn registry_itself_can_move_to_another_thread() {
        let mut registry = Registry::default();
        registry.register(std::sync::Arc::new(counter("x")));
        let result = thread::spawn(move || registry.poll_all_in_threads())
            .join()
            .expect("thread panicked");
        assert_eq!(result, vec!["x=1"]);
    }
}
