// c37_e02_signal_cli.rs
//
// CONCEPT: PROJECT, no scaffolding. A complete command-line program:
//          argument parsing, file I/O, parsing a recursive JSON format,
//          glob queries, formatted output, layered errors and exit codes.
// GUIDE:   chapters/37-capstone-projects/guide.md  (section "Project 2: a VSS signal CLI")
// DATABROKER: databroker/src/vss.rs (parse + flatten the VSS tree),
//             databroker/src/glob.rs `Matcher` (VSS glob → glob_match),
//             databroker/src/main.rs (clap, reading `--vss` files)
// EXPECT:  tests fail: `todo!()` panics
//
// THE SPEC
//   `run(args, stdout, stderr) -> exit code` behaves like the program
//   `vss <COMMAND>` (args[0] is the program name). Writing to the given
//   writers instead of the real stdout/stderr is what makes it testable.
//
//   The input file is a VSS tree in JSON. Every node has a "type"
//   ("branch", "sensor", "actuator" or "attribute"); branches have
//   "children" (an object of name -> node); other nodes have "datatype" and
//   may have "unit". A node's path is its ancestors' names joined with ".".
//
//   vss query <FILE> <GLOB>
//       One line per matching NON-branch node, sorted by path, fields
//       separated by a tab:  <path>\t<type>\t<datatype>[\t<unit>]
//       GLOB rules (as in the databroker): `*` matches one path segment,
//       `**` any number of segments; a GLOB without `*` matches that path
//       and everything below it. Exit 0 if something matched; otherwise
//       print `no signals match <GLOB>` to stderr and exit 1.
//   vss tree <FILE> [--depth <N>]
//       The tree, children sorted by name, two spaces of indent per level.
//       Branches print as `<name>/`, other nodes as `<name> (<datatype>)`.
//       `--depth N` prints only the first N levels (top level = 1).
//
//   Errors go to stderr as `error: <message>`, followed by one
//   `  caused by: <message>` line per underlying cause, and exit 2:
//       cannot read <FILE>          (then the I/O error as the cause)
//       invalid VSS in <FILE>       (then the JSON / format error as the cause)
//   Usage errors (unknown command, missing argument) print clap's message
//   to stderr and exit 2. `--help` prints help to stdout and exits 0.
//
// YOUR TASK
//   Design and implement it. Only `run`'s signature is fixed. Suggested
//   crates: clap (derive), serde/serde_json, glob-match, thiserror.
//   Do not edit the tests.

// I AM NOT DONE

#![allow(unused_imports)]
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn run(args: &[&str], stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let _ = (args, stdout, stderr);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VSS: &str = r#"{
      "Vehicle": {
        "type": "branch",
        "children": {
          "Speed": {"type": "sensor", "datatype": "float", "unit": "km/h"},
          "VIN": {"type": "attribute", "datatype": "string"},
          "Cabin": {
            "type": "branch",
            "children": {
              "Door": {
                "type": "branch",
                "children": {
                  "Row1": {
                    "type": "branch",
                    "children": {
                      "IsOpen": {"type": "sensor", "datatype": "boolean"},
                      "Window": {"type": "actuator", "datatype": "uint8", "unit": "percent"}
                    }
                  }
                }
              },
              "Temperature": {"type": "sensor", "datatype": "float", "unit": "celsius"}
            }
          }
        }
      }
    }"#;

    /// A uniquely named file in the temp directory, removed on drop.
    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str, contents: &str) -> TempFile {
            let path = std::env::temp_dir().join(format!(
                "learn_rust_c37_e02_{}_{name}.json",
                std::process::id()
            ));
            std::fs::write(&path, contents).expect("write temp file");
            TempFile(path)
        }

        fn arg(&self) -> String {
            self.0.display().to_string()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn vss(args: &[&str]) -> (i32, String, String) {
        let mut all = vec!["vss"];
        all.extend_from_slice(args);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&all, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("stdout is UTF-8"),
            String::from_utf8(err).expect("stderr is UTF-8"),
        )
    }

    #[test]
    fn query_with_wildcards() {
        let file = TempFile::new("wild", VSS);
        let (code, out, err) = vss(&["query", &file.arg(), "Vehicle.Cabin.**"]);
        assert_eq!(code, 0, "stderr: {err}");
        assert_eq!(
            out,
            "Vehicle.Cabin.Door.Row1.IsOpen\tsensor\tboolean\n\
             Vehicle.Cabin.Door.Row1.Window\tactuator\tuint8\tpercent\n\
             Vehicle.Cabin.Temperature\tsensor\tfloat\tcelsius\n"
        );
        let (code, out, _) = vss(&["query", &file.arg(), "Vehicle.*"]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            "Vehicle.Speed\tsensor\tfloat\tkm/h\nVehicle.VIN\tattribute\tstring\n"
        );
        let (_, out, _) = vss(&["query", &file.arg(), "**.IsOpen"]);
        assert_eq!(out, "Vehicle.Cabin.Door.Row1.IsOpen\tsensor\tboolean\n");
    }

    #[test]
    fn query_without_wildcards_matches_the_branch_below() {
        let file = TempFile::new("branch", VSS);
        let (code, out, _) = vss(&["query", &file.arg(), "Vehicle.Cabin.Door"]);
        assert_eq!(code, 0);
        assert_eq!(out.lines().count(), 2);
        let (_, out, _) = vss(&["query", &file.arg(), "Vehicle.Speed"]);
        assert_eq!(out, "Vehicle.Speed\tsensor\tfloat\tkm/h\n");
        // A prefix of a name is not a branch.
        let (code, _, err) = vss(&["query", &file.arg(), "Vehicle.Spe"]);
        assert_eq!(code, 1);
        assert_eq!(err, "no signals match Vehicle.Spe\n");
    }

    #[test]
    fn tree_prints_sorted_indented_nodes() {
        let file = TempFile::new("tree", VSS);
        let (code, out, _) = vss(&["tree", &file.arg()]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            "Vehicle/\n  Cabin/\n    Door/\n      Row1/\n        IsOpen (boolean)\n        \
             Window (uint8)\n    Temperature (float)\n  Speed (float)\n  VIN (string)\n"
        );
        let (_, out, _) = vss(&["tree", &file.arg(), "--depth", "2"]);
        assert_eq!(out, "Vehicle/\n  Cabin/\n  Speed (float)\n  VIN (string)\n");
    }

    #[test]
    fn missing_file_explains_the_cause() {
        let (code, out, err) = vss(&["query", "/nonexistent/learn-rust/vss.json", "**"]);
        assert_eq!(code, 2);
        assert!(out.is_empty());
        let mut lines = err.lines();
        assert_eq!(
            lines.next(),
            Some("error: cannot read /nonexistent/learn-rust/vss.json")
        );
        assert!(
            lines.next().is_some_and(|l| l.starts_with("  caused by: ")),
            "missing cause line in {err:?}"
        );
    }

    #[test]
    fn invalid_files_explain_what_is_wrong() {
        let broken = TempFile::new("broken", "{ not json");
        let (code, _, err) = vss(&["tree", &broken.arg()]);
        assert_eq!(code, 2);
        assert!(err.starts_with(&format!(
            "error: invalid VSS in {}\n  caused by: ",
            broken.arg()
        )));

        let no_type = TempFile::new(
            "no_datatype",
            r#"{"Vehicle": {"type": "branch", "children": {"Speed": {"type": "sensor"}}}}"#,
        );
        let (code, _, err) = vss(&["query", &no_type.arg(), "**"]);
        assert_eq!(code, 2);
        assert_eq!(
            err,
            format!(
                "error: invalid VSS in {}\n  caused by: Vehicle.Speed has no datatype\n",
                no_type.arg()
            )
        );

        let bad_kind = TempFile::new("bad_kind", r#"{"Vehicle": {"type": "gadget"}}"#);
        let (_, _, err) = vss(&["tree", &bad_kind.arg()]);
        assert!(
            err.ends_with("  caused by: Vehicle has unknown type \"gadget\"\n"),
            "{err}"
        );
    }

    #[test]
    fn usage_errors_and_help() {
        let (code, out, err) = vss(&["frobnicate"]);
        assert_eq!(code, 2);
        assert!(out.is_empty());
        assert!(err.contains("frobnicate"), "{err}");

        let (code, _, err) = vss(&["query"]);
        assert_eq!(code, 2);
        assert!(err.contains("required"), "{err}");

        let (code, out, err) = vss(&["--help"]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        assert!(out.contains("query") && out.contains("tree"), "{out}");
    }
}
