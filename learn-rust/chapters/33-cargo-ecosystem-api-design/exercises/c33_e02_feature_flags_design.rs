// c33_e02_feature_flags_design.rs
//
// CONCEPT: how Cargo resolves features: `default`, features enabling other
//          features, `dep:x` (turn on an optional dependency), `crate/feat`
//          (turn on a feature of a dependency), weak `crate?/feat`, and
//          feature *unification* across a whole build.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section "Features: additive switches, unified per build")
// DATABROKER: databroker/Cargo.toml:83-89 — `default = ["tls"]`,
//             `tls = ["tonic/tls-ring", ...]`, `viss = ["dep:axum", "dep:chrono"]`.
//             This test reads that real file.
// EXPECT:  tests fail: `todo!()` panics
//
// `cargo build --no-default-features --features viss` — what does that
// actually switch on? You'll write the resolver that answers it, then run it
// against the databroker's own manifest.
//
// YOUR TASK
//   Implement `resolve` and `unify`. `parse_manifest` is provided (it's a
//   deliberately tiny TOML reader that only understands what this file needs).
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::{BTreeMap, BTreeSet};

/// The parts of a Cargo.toml that matter for feature resolution.
#[derive(Debug, Default)]
pub struct Manifest {
    /// `[features]` table: feature name → list of entries.
    pub features: BTreeMap<String, Vec<String>>,
    /// Names of dependencies declared with `optional = true`.
    pub optional_deps: BTreeSet<String>,
}

/// Everything a set of requested features switches on.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    /// Features of *this* crate that are on (including `default` if used).
    pub features: BTreeSet<String>,
    /// Optional dependencies that get compiled in.
    pub optional_deps: BTreeSet<String>,
    /// Features switched on in dependencies, as `"crate/feature"`.
    pub dep_features: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FeatureError {
    /// A requested feature, or an entry naming a feature, doesn't exist.
    Unknown(String),
}

/// Resolves `requested` features (plus `default`, if `default_features`).
///
/// Rules (the same ones Cargo uses):
/// - `"default"` is an ordinary feature that's requested automatically
///   unless `default_features` is false. It's fine for it to be missing.
/// - An entry `"dep:x"` enables the optional dependency `x`.
/// - An entry `"x/f"` enables feature `f` of dependency `x`, AND if `x` is an
///   optional dependency it enables `x` too.
/// - An entry `"x?/f"` (weak) enables feature `f` of `x` ONLY if `x` ends up
///   enabled some other way. It never enables `x` itself.
/// - Any other entry names another feature of this crate (recursively).
///   Unknown names → `FeatureError::Unknown`.
/// - Features can reference each other in cycles; don't loop forever.
pub fn resolve(
    manifest: &Manifest,
    requested: &[&str],
    default_features: bool,
) -> Result<Resolved, FeatureError> {
    todo!()
}

/// Feature unification: when several crates in one build depend on the same
/// crate with different feature sets, Cargo compiles it ONCE with the union.
pub fn unify(requests: &[BTreeSet<String>]) -> BTreeSet<String> {
    todo!()
}

/// Provided: reads `[features]` and `optional = true` dependencies.
/// Only understands single-line `name = [...]` arrays and single-line
/// dependency tables, which is all the databroker's manifest uses.
pub fn parse_manifest(toml: &str) -> Manifest {
    let mut manifest = Manifest::default();
    let mut section = String::new();
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        if section == "features" {
            let entries = value
                .split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect();
            manifest.features.insert(key, entries);
        } else if section.ends_with("dependencies") && value.contains("optional = true") {
            manifest.optional_deps.insert(key);
        }
    }
    manifest
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATABROKER_TOML: &str = include_str!("../../../../databroker/Cargo.toml");

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn databroker() -> Manifest {
        parse_manifest(DATABROKER_TOML)
    }

    #[test]
    fn parser_sees_the_real_manifest() {
        let m = databroker();
        assert_eq!(m.features["default"], vec!["tls"]);
        assert!(m.optional_deps.contains("axum"));
        assert!(!m.optional_deps.contains("tokio"));
    }

    #[test]
    fn default_build_of_the_databroker() {
        let r = resolve(&databroker(), &[], true).expect("resolves");
        assert_eq!(r.features, set(&["default", "tls"]));
        assert_eq!(
            r.dep_features,
            set(&["kuksa-common/tls", "kuksa/tls", "tonic/tls-ring"])
        );
        assert!(r.optional_deps.is_empty());
    }

    #[test]
    fn no_default_features_with_viss_and_otel() {
        let r = resolve(&databroker(), &["viss", "otel"], false).expect("resolves");
        assert_eq!(r.features, set(&["otel", "viss"]));
        assert_eq!(
            r.optional_deps,
            set(&[
                "axum",
                "chrono",
                "opentelemetry",
                "opentelemetry-otlp",
                "opentelemetry_sdk",
                "tracing-opentelemetry",
            ])
        );
        assert!(
            r.dep_features.is_empty(),
            "tls is off: {:?}",
            r.dep_features
        );
    }

    #[test]
    fn unknown_feature_is_an_error() {
        assert_eq!(
            resolve(&databroker(), &["turbo"], true),
            Err(FeatureError::Unknown("turbo".to_string()))
        );
    }

    fn synthetic() -> Manifest {
        let mut m = Manifest::default();
        let mut add = |name: &str, entries: &[&str]| {
            m.features.insert(
                name.to_string(),
                entries.iter().map(|s| s.to_string()).collect(),
            );
        };
        add("default", &["std"]);
        add("std", &["serde?/std", "a"]);
        add("serde", &["dep:serde", "serde_json/preserve_order"]);
        add("a", &["b"]);
        add("b", &["a"]); // a cycle
        add("broken", &["missing"]);
        m.optional_deps = set(&["serde", "serde_json"]);
        m
    }

    #[test]
    fn cycles_terminate_and_nested_unknowns_are_errors() {
        let r = resolve(&synthetic(), &[], true).expect("resolves");
        assert_eq!(r.features, set(&["a", "b", "default", "std"]));
        assert_eq!(
            resolve(&synthetic(), &["broken"], false),
            Err(FeatureError::Unknown("missing".to_string()))
        );
    }

    #[test]
    fn weak_features_do_not_enable_their_dependency() {
        // serde is optional and nothing enables it: `serde?/std` does nothing.
        let r = resolve(&synthetic(), &[], true).expect("resolves");
        assert!(r.optional_deps.is_empty());
        assert!(r.dep_features.is_empty());

        // Once `serde` is on, the weak entry applies. And `serde_json/...`
        // turns on the optional serde_json dependency by itself.
        let r = resolve(&synthetic(), &["serde"], true).expect("resolves");
        assert_eq!(r.optional_deps, set(&["serde", "serde_json"]));
        assert_eq!(
            r.dep_features,
            set(&["serde/std", "serde_json/preserve_order"])
        );
    }

    #[test]
    fn unification_is_a_union() {
        // Two crates in one build both depend on `kuksa`:
        let from_databroker = set(&["tls"]);
        let from_some_tool = set(&["verbose-logs"]);
        assert_eq!(
            unify(&[from_databroker, from_some_tool]),
            set(&["tls", "verbose-logs"])
        );
        assert!(unify(&[]).is_empty());
    }
}
