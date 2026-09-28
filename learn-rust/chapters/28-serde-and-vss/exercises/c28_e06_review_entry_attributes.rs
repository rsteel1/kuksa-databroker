// c28_e06_review_entry_attributes.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs typical
//          of chapter 28 (a `#[serde(rename)]` that doesn't match the real
//          JSON key, a `#[serde(default = "...")]` whose default function
//          returns the wrong value, and an enum variant renamed to the
//          wrong case so a value that's actually present in the JSON fails
//          to parse).
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "Renaming fields
//          and defaults")
// DATABROKER: databroker/src/vss.rs:39-61 — `Entry` renames `entry_type`
//             from `"type"`, `data_type` from `"datatype"`, and
//             `change_type` from `"x-kuksa-changetype"`; a typo in any of
//             those strings makes the field silently report itself as
//             missing (Option) or fail to deserialize at all (required).
// EXPECT:  tests fail at runtime: a "sensor" entry reports its type as
//          missing, a missing `deprecated` key defaults to `true` instead
//          of `false`, and a present "continuous" change type fails to
//          parse.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not rename any Rust field or variant, and do not edit the tests.

// I AM NOT DONE

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
enum ChangeType {
    #[serde(rename = "static")]
    Static,
    #[serde(rename = "onchange")]
    OnChange,
    #[serde(rename = "Continuous")]
    Continuous,
}

fn default_deprecated() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct VssEntry {
    #[serde(rename = "Type")]
    entry_type: String,
    description: String,
    #[serde(rename = "datatype")]
    data_type: Option<String>,
    #[serde(rename = "x-kuksa-changetype")]
    change_type: Option<ChangeType>,
    #[serde(default = "default_deprecated")]
    deprecated: bool,
}

fn parse(json: &str) -> Result<VssEntry, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_with_all_keys() {
        let json = r#"{
            "type": "sensor",
            "description": "Vehicle speed",
            "datatype": "float",
            "x-kuksa-changetype": "continuous",
            "deprecated": true
        }"#;
        let e = parse(json).expect("valid entry");
        assert_eq!(e.entry_type, "sensor");
        assert_eq!(e.data_type.as_deref(), Some("float"));
        assert_eq!(e.change_type, Some(ChangeType::Continuous));
        assert!(e.deprecated);
    }

    #[test]
    fn branch_has_no_datatype_and_defaults_to_not_deprecated() {
        let json = r#"{ "type": "branch", "description": "Cabin" }"#;
        let e = parse(json).expect("valid branch");
        assert_eq!(e.entry_type, "branch");
        assert_eq!(e.description, "Cabin");
        assert_eq!(e.data_type, None);
        assert_eq!(e.change_type, None);
        assert!(
            !e.deprecated,
            "a missing `deprecated` key must default to false, not true"
        );
    }

    #[test]
    fn rust_field_name_is_not_accepted_as_a_json_key() {
        // After renaming, the JSON key "entry_type" is simply unknown
        // (ignored), so the real key "type" is missing.
        let json = r#"{ "entry_type": "sensor", "description": "x" }"#;
        let err = parse(json).expect_err("`type` is required");
        assert!(err.to_string().contains("missing field `type`"));
    }
}
