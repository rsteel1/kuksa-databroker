// c12_e07_oncelock_lazylock.rs
//
// CONCEPT: global values that need runtime initialisation — `LazyLock`
//          (init on first use) and `OnceLock` (init once, from wherever).
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Globals: LazyLock, OnceLock and lazy_static")
// DATABROKER: databroker/src/permissions.rs — `lazy_static! { pub static ref
//          ALLOW_ALL: Permissions = ... }` (line 21). `lazy_static` predates
//          std's `LazyLock` (stable since Rust 1.80); new code uses std.
// EXPECT:  compile error E0015 (cannot call non-const fn in statics).
//
// A `static` is initialised at COMPILE time, so its initialiser must be a
// `const` expression. Building a `Vec`/`String`/`HashMap` isn't. `LazyLock`
// stores a closure and runs it the first time the value is touched —
// thread-safely, exactly once.
//
// YOUR TASK
//   1. Make `ALLOW_ALL` a `LazyLock<Permissions>`.
//   2. Implement `config()` with a `static OnceLock<Config>` so
//      `load_config` runs only once no matter how many threads call it.
//   Do not edit the tests or `load_config`.

// I AM NOT DONE

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct Permissions {
    readable: Vec<String>,
}

impl Permissions {
    fn all() -> Self {
        Permissions {
            readable: vec!["**".to_string()],
        }
    }

    fn can_read_everything(&self) -> bool {
        self.readable.iter().any(|g| g == "**")
    }
}

static ALLOW_ALL: Permissions = Permissions::all();

#[derive(Debug)]
struct Config {
    port: u16,
}

static LOADS: AtomicUsize = AtomicUsize::new(0);

fn load_config() -> Config {
    LOADS.fetch_add(1, Ordering::SeqCst);
    Config { port: 55555 }
}

/// Returns the process-wide config, loading it on first call.
fn config() -> &'static Config {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_all_is_usable_everywhere() {
        assert!(ALLOW_ALL.can_read_everything());
        let from_thread = std::thread::spawn(|| ALLOW_ALL.readable.len())
            .join()
            .expect("thread panicked");
        assert_eq!(from_thread, 1);
    }

    #[test]
    fn config_is_loaded_exactly_once() {
        let ports: Vec<u16> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8).map(|_| s.spawn(|| config().port)).collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("thread panicked"))
                .collect()
        });
        assert!(ports.iter().all(|p| *p == 55555));
        assert!(std::ptr::eq(config(), config()));
        assert_eq!(LOADS.load(Ordering::SeqCst), 1);
    }
}
