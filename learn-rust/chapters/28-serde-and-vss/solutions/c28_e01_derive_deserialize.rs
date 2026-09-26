// c28_e01_derive_deserialize.rs
//
// CONCEPT: `#[derive(Deserialize)]` turns JSON into a Rust struct.
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "Deriving Deserialize")
// DATABROKER: databroker/src/vss.rs — `struct Entry` with `#[derive(Debug, Deserialize)]`
// EXPECT:  compile error E0277: the trait bound `SignalInfo: Deserialize<'_>` is not satisfied
//
// The databroker learns which signals exist by reading a VSS JSON file at
// startup (`--vss vss.json`). serde does the heavy lifting: you describe the
// shape of the data as a struct, derive `Deserialize`, and `serde_json`
// generates the parser for you. Optional JSON fields become `Option<T>`.
//
// YOUR TASK
//   Make `SignalInfo` deserializable so `parse_signal` compiles and the
//   tests pass. You only need to change the struct definition.
//   Do not edit the tests.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SignalInfo {
    description: String,
    datatype: String,
    // Not every signal has a unit (e.g. booleans), so this is optional.
    unit: Option<String>,
}

fn parse_signal(json: &str) -> Result<SignalInfo, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_fields() {
        let json = r#"{ "description": "Vehicle speed", "datatype": "float", "unit": "km/h" }"#;
        let info = parse_signal(json).expect("valid JSON");
        assert_eq!(info.description, "Vehicle speed");
        assert_eq!(info.datatype, "float");
        assert_eq!(info.unit.as_deref(), Some("km/h"));
    }

    #[test]
    fn missing_optional_field_is_none() {
        let json = r#"{ "description": "Door open?", "datatype": "boolean" }"#;
        let info = parse_signal(json).expect("unit is optional");
        assert_eq!(info.unit, None);
    }

    #[test]
    fn missing_required_field_is_an_error() {
        let json = r#"{ "datatype": "float" }"#;
        let err = parse_signal(json).expect_err("description is required");
        assert!(err.to_string().contains("missing field `description`"));
    }

    #[test]
    fn unknown_fields_are_ignored_by_default() {
        let json = r#"{ "description": "d", "datatype": "float", "uuid": "abc" }"#;
        assert!(parse_signal(json).is_ok());
    }
}
