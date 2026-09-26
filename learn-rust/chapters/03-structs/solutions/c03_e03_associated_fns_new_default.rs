// c03_e03_associated_fns_new_default.rs
//
// CONCEPT: constructors (`new`), the `Default` trait — derived and hand-written.
// GUIDE:   chapters/03-structs/guide.md  (section "Constructors: new and Default")
// DATABROKER: databroker/src/broker.rs — `DataBroker::new(version, commit_sha)`
//             and `impl Default for DataBroker` (calls `Self::new("", "")`);
//             `#[derive(Default)] pub struct Database`.
// EXPECT:  compile errors E0599 (no function or associated item named `new` /
//          `default` found).
//
// Rust has no special constructor syntax. By convention a type gets an
// associated function `new`. If a type has an obvious "empty" or "standard"
// value it also implements `Default`, which lets it work with
// `..Default::default()`, `unwrap_or_default()` and `#[derive(Default)]` on
// structs that contain it.
//
// YOUR TASK
//   1. Add `ServerSettings::new(address: &str, port: u16)`.
//   2. Implement `Default` for `ServerSettings` by hand: address "127.0.0.1",
//      port 55555 (the databroker's defaults), `enable_unix_socket` false.
//   3. Make `Counters` default-constructible with `#[derive(Default)]`.
//   Do not edit the tests.

#[derive(Debug)]
struct ServerSettings {
    address: String,
    port: u16,
    enable_unix_socket: bool,
}

impl ServerSettings {
    fn new(address: &str, port: u16) -> Self {
        // WHY: `Self` is an alias for the type the impl block is for.
        Self {
            address: address.to_string(),
            port,
            enable_unix_socket: false,
        }
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        // WHY: reuse `new` so the two constructors can never drift apart
        // (the same trick as `impl Default for DataBroker`).
        Self::new("127.0.0.1", 55555)
    }
}

// WHY: every field (u64, u64, Vec) has a Default, so the derive works:
// it sets each field to *its* default (0, 0, empty Vec).
#[derive(Debug, Default)]
struct Counters {
    published: u64,
    dropped: u64,
    last_errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_constructor() {
        let settings = ServerSettings::new("0.0.0.0", 8080);
        assert_eq!(settings.address, "0.0.0.0");
        assert_eq!(settings.port, 8080);
        assert!(!settings.enable_unix_socket);
    }

    #[test]
    fn default_settings() {
        let settings = ServerSettings::default();
        assert_eq!(settings.address, "127.0.0.1");
        assert_eq!(settings.port, 55555);
    }

    #[test]
    fn derived_default() {
        let counters = Counters::default();
        assert_eq!(counters.published, 0);
        assert_eq!(counters.dropped, 0);
        assert!(counters.last_errors.is_empty());
        let other: Counters = Default::default();
        assert_eq!(other.published, 0);
    }
}
