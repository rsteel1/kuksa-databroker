// c33_e05_choosing_crates.rs
//
// CONCEPT: reading a dependency graph: `cargo tree`, inverted trees
//          (`cargo tree -i`), duplicate versions (`cargo tree -d`),
//          proc-macro and local path dependencies, and what that tells you
//          before you add (or audit) a crate.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section "Choosing and auditing crates")
// DATABROKER: databroker/Cargo.toml — the real output of
//             `cargo tree -p databroker -e normal --depth 2` and an excerpt of
//             `cargo tree -p databroker -e normal -d` are embedded below.
// EXPECT:  tests fail: `todo!()` panics
//
// Before adding a crate you should be able to answer: what does it pull in,
// do we already have another version of it, is it a proc-macro (compile
// time only, runs code at build time), and who else depends on it? Every
// answer is in `cargo tree` if you can read it. Here you'll teach a program
// to read it, which forces you to understand the format exactly.
//
// YOUR TASK
//   Implement `parse_line` and the query functions below it. The format:
//   each level of nesting is 4 characters of prefix drawn from "│   ",
//   "├── ", "└── " and spaces; then `name vVERSION`, optionally followed by
//   `(path)` for local crates, `(proc-macro)`, and `(*)` when the subtree
//   was already printed above. Do not edit the tests or the embedded outputs.

// I AM NOT DONE

use std::collections::{BTreeMap, BTreeSet};

/// One line of `cargo tree` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// 0 for the root, 1 for its direct children, ...
    pub depth: usize,
    pub name: String,
    /// Without the leading `v`, e.g. `"1.53.1"`.
    pub version: String,
    /// `Some("../lib/kuksa")` for path dependencies.
    pub local_path: Option<String>,
    pub proc_macro: bool,
    /// The line ended with `(*)`: its children were shown earlier.
    pub repeated: bool,
}

/// Parses one non-empty line. Blank lines → `None`.
pub fn parse_line(line: &str) -> Option<Node> {
    todo!()
}

pub fn parse_tree(text: &str) -> Vec<Node> {
    text.lines().filter_map(parse_line).collect()
}

/// Names of the root's direct dependencies, in output order.
pub fn direct_deps(nodes: &[Node]) -> Vec<String> {
    todo!()
}

/// Every proc-macro crate that appears anywhere in the tree.
pub fn proc_macros(nodes: &[Node]) -> BTreeSet<String> {
    todo!()
}

/// Every local (path) crate, with its path.
pub fn local_crates(nodes: &[Node]) -> BTreeMap<String, String> {
    todo!()
}

/// All distinct versions of `name` seen in the tree, sorted as strings.
pub fn versions_of(nodes: &[Node], name: &str) -> BTreeSet<String> {
    todo!()
}

/// For `cargo tree -d` output (several inverted trees separated by blank
/// lines): crate name → every duplicated version, in output order.
pub fn duplicates(text: &str) -> BTreeMap<String, Vec<String>> {
    todo!()
}

/// In inverted output, the children of a root are the crates that depend on
/// it directly. Returns them for the root `name vVERSION`.
pub fn direct_dependents(text: &str, name: &str, version: &str) -> Vec<String> {
    todo!()
}

/// The first complete chain from the root `name vVERSION` down to a crate
/// with no dependents (the workspace crate): follow each node's FIRST child.
/// E.g. `["socket2", "tokio", "databroker"]`.
pub fn first_chain(text: &str, name: &str, version: &str) -> Vec<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo tree -p databroker -e normal --depth 2` (absolute paths shortened to `..`).
    const TREE: &str = "\
databroker v0.7.2-dev.0 (../databroker)
├── async-stream v0.3.6
│   ├── async-stream-impl v0.3.6 (proc-macro)
│   ├── futures-core v0.3.34
│   └── pin-project-lite v0.2.17
├── async-trait v0.1.92 (proc-macro)
│   ├── proc-macro2 v1.0.107
│   ├── quote v1.0.47
│   └── syn v3.0.4
├── clap v4.6.6
│   └── clap_builder v4.6.6
├── databroker-proto v0.7.2-dev.0 (../databroker-proto)
│   ├── prost v0.14.4
│   ├── prost-types v0.14.4
│   ├── tonic v0.14.6
│   └── tonic-prost v0.14.6
├── futures v0.3.34
│   ├── futures-channel v0.3.34
│   ├── futures-core v0.3.34
│   ├── futures-executor v0.3.34
│   ├── futures-io v0.3.34
│   ├── futures-sink v0.3.34
│   ├── futures-task v0.3.34
│   └── futures-util v0.3.34
├── glob-match v0.2.1
├── indexmap v2.14.0
│   ├── equivalent v1.0.2
│   └── hashbrown v0.17.1
├── jsonwebtoken v10.4.0
│   ├── base64 v0.22.1
│   ├── ed25519-dalek v2.2.0
│   ├── hmac v0.12.1
│   ├── p256 v0.13.2
│   ├── p384 v0.13.1
│   ├── pem v3.0.6
│   ├── rand v0.8.8
│   ├── rsa v0.9.10
│   ├── serde v1.0.229
│   ├── serde_json v1.0.151
│   ├── sha2 v0.10.9
│   ├── signature v2.2.0
│   ├── simple_asn1 v0.6.4
│   └── zeroize v1.9.0
├── kuksa v0.7.2-dev.0 (../lib/kuksa)
│   ├── databroker-proto v0.7.2-dev.0 (../databroker-proto) (*)
│   ├── http v1.5.0
│   ├── kuksa-common v0.7.2-dev.0 (../lib/common)
│   ├── tokio v1.53.1
│   ├── tokio-stream v0.1.19
│   └── tonic v0.14.6 (*)
├── kuksa-common v0.7.2-dev.0 (../lib/common) (*)
├── lazy_static v1.5.0
│   └── spin v0.9.9
├── prost v0.14.4 (*)
├── prost-types v0.14.4 (*)
├── regex v1.13.1
│   ├── aho-corasick v1.1.5
│   ├── memchr v2.8.3
│   ├── regex-automata v0.4.18
│   └── regex-syntax v0.8.11
├── serde v1.0.229 (*)
├── serde_json v1.0.151 (*)
├── socket2 v0.5.10
│   └── libc v0.2.189
├── thiserror v1.0.69
│   └── thiserror-impl v1.0.69 (proc-macro)
├── tokio v1.53.1 (*)
├── tokio-stream v0.1.19 (*)
├── tonic v0.14.6 (*)
├── tonic-reflection v0.14.6
│   ├── prost v0.14.4 (*)
│   ├── prost-types v0.14.4 (*)
│   ├── tokio v1.53.1 (*)
│   ├── tokio-stream v0.1.19 (*)
│   ├── tonic v0.14.6 (*)
│   └── tonic-prost v0.14.6 (*)
├── tracing v0.1.44
│   ├── pin-project-lite v0.2.17
│   ├── tracing-attributes v0.1.31 (proc-macro)
│   └── tracing-core v0.1.36
├── tracing-subscriber v0.3.23
│   ├── matchers v0.2.0
│   ├── nu-ansi-term v0.50.3
│   ├── once_cell v1.21.4
│   ├── regex-automata v0.4.18 (*)
│   ├── sharded-slab v0.1.7
│   ├── thread_local v1.1.10
│   ├── tracing v0.1.44 (*)
│   └── tracing-core v0.1.36 (*)
└── uuid v1.25.0
    └── getrandom v0.4.3
";

    /// Excerpt of `cargo tree -p databroker -e normal -d` (some branches trimmed).
    const DUPLICATES: &str = "\
getrandom v0.2.17
├── rand_core v0.6.4
│   ├── crypto-bigint v0.5.5
│   │   └── elliptic-curve v0.13.8
│   │       ├── ecdsa v0.16.9
│   │       │   ├── p256 v0.13.2
│   │       │   │   └── jsonwebtoken v10.4.0
│   │       │   │       └── databroker v0.7.2-dev.0 (../databroker)
│   │       │   └── p384 v0.13.1
│   │       │       └── jsonwebtoken v10.4.0 (*)
│   │       └── primeorder v0.13.6
│   │           ├── p256 v0.13.2 (*)
│   │           └── p384 v0.13.1 (*)
│   └── rand v0.8.8
│       └── jsonwebtoken v10.4.0 (*)
└── ring v0.17.14
    └── rustls v0.23.43
        └── tokio-rustls v0.26.4
            └── tonic v0.14.6
                └── databroker v0.7.2-dev.0 (../databroker)

getrandom v0.4.3
└── uuid v1.25.0
    └── databroker v0.7.2-dev.0 (../databroker)

socket2 v0.5.10
└── databroker v0.7.2-dev.0 (../databroker)

socket2 v0.6.5
├── hyper-util v0.1.20
│   ├── hyper-timeout v0.5.2
│   │   └── tonic v0.14.6 (*)
│   └── tonic v0.14.6 (*)
└── tokio v1.53.1
    └── databroker v0.7.2-dev.0 (../databroker)
";

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_single_lines() {
        assert_eq!(parse_line(""), None);
        assert_eq!(
            parse_line("├── async-trait v0.1.92 (proc-macro)"),
            Some(Node {
                depth: 1,
                name: "async-trait".to_string(),
                version: "0.1.92".to_string(),
                local_path: None,
                proc_macro: true,
                repeated: false,
            })
        );
        assert_eq!(
            parse_line("│   ├── databroker-proto v0.7.2-dev.0 (../databroker-proto) (*)"),
            Some(Node {
                depth: 2,
                name: "databroker-proto".to_string(),
                version: "0.7.2-dev.0".to_string(),
                local_path: Some("../databroker-proto".to_string()),
                proc_macro: false,
                repeated: true,
            })
        );
        let root = parse_line("databroker v0.7.2-dev.0 (../databroker)").expect("root parses");
        assert_eq!(root.depth, 0);
    }

    #[test]
    fn direct_dependencies_of_the_databroker() {
        let nodes = parse_tree(TREE);
        let direct = direct_deps(&nodes);
        assert_eq!(direct.len(), 25);
        assert_eq!(direct.first().map(String::as_str), Some("async-stream"));
        assert_eq!(direct.last().map(String::as_str), Some("uuid"));
        assert!(direct.contains(&"jsonwebtoken".to_string()));
        assert!(
            !direct.contains(&"hyper".to_string()),
            "hyper is only indirect"
        );
    }

    #[test]
    fn proc_macros_and_local_crates() {
        let nodes = parse_tree(TREE);
        assert_eq!(
            proc_macros(&nodes),
            set(&[
                "async-stream-impl",
                "async-trait",
                "thiserror-impl",
                "tracing-attributes"
            ])
        );
        let local = local_crates(&nodes);
        assert_eq!(local["kuksa"], "../lib/kuksa");
        assert_eq!(local["databroker"], "../databroker");
        assert_eq!(local.len(), 4);
    }

    #[test]
    fn versions_in_the_tree() {
        let nodes = parse_tree(TREE);
        assert_eq!(versions_of(&nodes, "tokio"), set(&["1.53.1"]));
        assert_eq!(versions_of(&nodes, "getrandom"), set(&["0.4.3"]));
        assert!(versions_of(&nodes, "left-pad").is_empty());
    }

    #[test]
    fn duplicated_crates() {
        let dupes = duplicates(DUPLICATES);
        assert_eq!(dupes["getrandom"], ["0.2.17", "0.4.3"]);
        assert_eq!(dupes["socket2"], ["0.5.10", "0.6.5"]);
        assert_eq!(dupes.len(), 2);
    }

    #[test]
    fn who_depends_on_what() {
        assert_eq!(
            direct_dependents(DUPLICATES, "getrandom", "0.2.17"),
            ["rand_core", "ring"]
        );
        assert_eq!(
            direct_dependents(DUPLICATES, "socket2", "0.5.10"),
            ["databroker"]
        );
        assert_eq!(
            direct_dependents(DUPLICATES, "socket2", "0.6.5"),
            ["hyper-util", "tokio"]
        );
        assert!(direct_dependents(DUPLICATES, "socket2", "9.9.9").is_empty());
    }

    #[test]
    fn chains_explain_why_a_crate_is_in_the_build() {
        assert_eq!(
            first_chain(DUPLICATES, "getrandom", "0.2.17"),
            [
                "getrandom",
                "rand_core",
                "crypto-bigint",
                "elliptic-curve",
                "ecdsa",
                "p256",
                "jsonwebtoken",
                "databroker"
            ]
        );
        assert_eq!(
            first_chain(DUPLICATES, "getrandom", "0.4.3"),
            ["getrandom", "uuid", "databroker"]
        );
        assert!(first_chain(DUPLICATES, "nope", "1.0.0").is_empty());
    }
}
