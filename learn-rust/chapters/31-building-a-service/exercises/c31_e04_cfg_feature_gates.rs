// c31_e04_cfg_feature_gates.rs
//
// CONCEPT: conditional compilation — `#[cfg(...)]` on items, fields and parameters; `cfg!(...)`.
// GUIDE:   chapters/31-building-a-service/guide.md  (section "Conditional compilation: features and platforms")
// DATABROKER: databroker/src/main.rs — `#[cfg(target_os = "linux")]` sd-notify block,
//             `#[cfg(feature = "tls")] let tls_config`, `#[cfg(feature = "viss")]` block;
//             grpc/server.rs — `#[cfg(feature = "tls")] server_tls: ServerTLS` (a cfg'd PARAMETER)
// EXPECT:  compile error E0428: the name `readiness_notifier` is defined multiple times
//
// The databroker binary is built in many variants: with/without TLS, VISS,
// OpenTelemetry; on Linux (systemd readiness notification via sd-notify) or
// not. `#[cfg(...)]` removes an item *before type checking*, so code for a
// disabled feature or another OS doesn't even need to compile.
//
//   #[cfg(pred)] item        item exists only when pred is true
//   cfg!(pred)               a plain `bool` expression; BOTH branches must compile
//   #[cfg_attr(pred, attr)]  apply `attr` only when pred is true
//
// Predicates: `unix`, `windows`, `target_os = "linux"`, `debug_assertions`,
// `test`, `feature = "tls"`, and combinators `not(..)`, `all(..)`, `any(..)`.
// (This exercise sticks to platform predicates: this crate declares no
// features, and cargo warns about unknown `feature = ".."` names.)
//
// YOUR TASK
//   1. Two definitions of `readiness_notifier` exist. Gate them so the
//      "systemd" one only exists on Linux and the "noop" one everywhere else.
//   2. Gate the `unix_socket` FIELD (and its initialiser) to `unix` only.
//   3. Implement `build_kind` with `cfg!(debug_assertions)`.
//   Do not edit the tests.

// I AM NOT DONE

fn readiness_notifier() -> &'static str {
    "systemd"
}

fn readiness_notifier() -> &'static str {
    "noop"
}

#[derive(Debug)]
struct ServerConfig {
    port: u16,
    unix_socket: Option<String>,
}

fn default_config() -> ServerConfig {
    ServerConfig {
        port: 55555,
        unix_socket: Some("/run/kuksa/databroker.sock".to_string()),
    }
}

/// "debug" in debug builds (`cargo test` default), "release" otherwise.
fn build_kind() -> &'static str {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_uses_systemd() {
        assert_eq!(readiness_notifier(), "systemd");
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn other_platforms_do_nothing() {
        assert_eq!(readiness_notifier(), "noop");
    }

    #[test]
    fn config_has_platform_specific_fields() {
        let config = default_config();
        assert_eq!(config.port, 55555);
        #[cfg(unix)]
        assert_eq!(
            config.unix_socket.as_deref(),
            Some("/run/kuksa/databroker.sock")
        );
    }

    #[test]
    fn build_kind_matches_profile() {
        let expected = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        assert_eq!(build_kind(), expected);
    }
}
