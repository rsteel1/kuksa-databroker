// c08_e04_cfg_attributes.rs
//
// CONCEPT: conditional compilation: `#[cfg(...)]`, `#[cfg(not(...))]`,
//          `#[cfg_attr(..., ...)]`, and the `cfg!(...)` macro.
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Conditional compilation and features")
// DATABROKER: databroker/src/lib.rs — two versions of `init_logging`, one under
//             `#[cfg(feature = "otel")]`, one under `#[cfg(not(feature = "otel"))]`;
//             databroker/src/main.rs — `#[cfg(target_os = "linux")]` around the
//             systemd `sd_notify` call; broker.rs — `#[cfg_attr(feature = "otel",
//             tracing::instrument(...))]` on many methods.
// EXPECT:  compile errors E0428 (`build_kind` defined multiple times) and
//          E0369 (binary operation `==` cannot be applied to `Snapshot`)
//
// `#[cfg(X)]` removes the item entirely unless X is true *at compile time*.
// The code under a false cfg isn't type-checked or compiled at all.
// `cfg!(X)` is different: it's an ordinary `true`/`false` expression, so
// both branches must compile.
//
// Common predicates: `test`, `debug_assertions` (on in dev builds, off in
// --release), `target_os = "linux"`, `unix`, `feature = "otel"` (Cargo
// features — this course's crate declares none, so we use the others).
//
// YOUR TASK
//   1. Put `#[cfg(debug_assertions)]` / `#[cfg(not(debug_assertions))]` on the
//      two `build_kind` functions so exactly one exists in any build.
//   2. Make `Snapshot` derive `PartialEq` *only in test builds*, using
//      `#[cfg_attr(test, derive(PartialEq))]`.
//   3. Implement `readiness_mechanism` with the `cfg!` macro.
//   4. Gate `fake_snapshot` so it only exists in test builds.
//   Do not edit the tests.

#[cfg(debug_assertions)]
fn build_kind() -> &'static str {
    "debug"
}

#[cfg(not(debug_assertions))]
fn build_kind() -> &'static str {
    "release"
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
struct Snapshot {
    signals: usize,
    version: String,
}

/// On Linux the databroker tells systemd it's ready ("sd_notify");
/// elsewhere it does nothing ("none").
fn readiness_mechanism() -> &'static str {
    if cfg!(target_os = "linux") {
        "sd_notify"
    } else {
        "none"
    }
}

/// Test-only helper: must not exist in normal builds.
#[cfg(test)]
fn fake_snapshot() -> Snapshot {
    Snapshot {
        signals: 3,
        version: "test".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_build_kind() {
        // `cargo test` builds with debug assertions on.
        assert_eq!(build_kind(), "debug");
    }

    #[test]
    fn snapshots_compare_in_tests() {
        let a = fake_snapshot();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn readiness_depends_on_os() {
        let expected = if cfg!(target_os = "linux") {
            "sd_notify"
        } else {
            "none"
        };
        assert_eq!(readiness_mechanism(), expected);
    }
}
