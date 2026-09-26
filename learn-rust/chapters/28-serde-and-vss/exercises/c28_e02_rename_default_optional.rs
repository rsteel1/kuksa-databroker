// c28_e02_rename_default_optional.rs
//
// CONCEPT: field attributes — `#[serde(rename = "...")]`, `#[serde(default)]`.
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "Renaming fields and defaults")
// DATABROKER: databroker/src/vss.rs — `#[serde(rename = "type")] entry_type`,
//             `#[serde(rename = "x-kuksa-changetype")] change_type`
// EXPECT:  tests fail at runtime: "missing field `entry_type`"
//
// JSON keys don't always make good Rust names. VSS uses the key "type" (a
// Rust keyword!), "datatype" (Rust style would be `data_type`) and the
// KUKSA extension key "x-kuksa-changetype" (dashes aren't allowed in
// identifiers). serde lets the Rust field name differ from the JSON key.
//
// YOUR TASK
//   Add serde attributes to `VssEntry` so that:
//     * `entry_type`  is read from JSON key "type"
//     * `data_type`   is read from "datatype" (optional)
//     * `change_type` is read from "x-kuksa-changetype" (optional)
//     * `deprecated`  is `false` when the key is missing (hint: `default`)
//   Do not rename the Rust fields. Do not edit the tests.

// I AM NOT DONE

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct VssEntry {
    entry_type: String,
    description: String,
    data_type: Option<String>,
    change_type: Option<String>,
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
        assert_eq!(e.change_type.as_deref(), Some("continuous"));
        assert!(e.deprecated);
    }

    #[test]
    fn branch_has_no_datatype_and_defaults() {
        let json = r#"{ "type": "branch", "description": "Cabin" }"#;
        let e = parse(json).expect("valid branch");
        assert_eq!(e.entry_type, "branch");
        assert_eq!(e.description, "Cabin");
        assert_eq!(e.data_type, None);
        assert_eq!(e.change_type, None);
        assert!(!e.deprecated, "missing `deprecated` must default to false");
    }

    #[test]
    fn rust_field_name_is_not_accepted_as_key() {
        // After renaming, the JSON key "entry_type" is simply unknown (ignored),
        // so "type" is missing.
        let json = r#"{ "entry_type": "sensor", "description": "x" }"#;
        let err = parse(json).expect_err("`type` is required");
        assert!(err.to_string().contains("missing field `type`"));
    }
}
