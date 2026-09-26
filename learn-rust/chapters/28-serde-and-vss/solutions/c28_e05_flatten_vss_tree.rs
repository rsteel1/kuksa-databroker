// c28_e05_flatten_vss_tree.rs
//
// CONCEPT: recursive deserialization + recursive flattening into a BTreeMap.
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "From a tree to flat paths")
// DATABROKER: databroker/src/vss.rs — `RootEntry`, `flatten_vss_tree`, `add_entry`,
//             `parse_vss_from_str`
// EXPECT:  tests fail: `todo!()` panics
//
// A VSS file is a tree: branches have `children`, leaves (sensor, actuator,
// attribute) have a `datatype`. The databroker stores signals by their full
// dotted path ("Vehicle.Cabin.Door.IsOpen"), so after parsing it walks the
// tree and builds a flat, sorted `BTreeMap<String, DataEntry>`.
//
// Note `struct Entry` contains `Option<HashMap<String, Entry>>` — a type that
// contains itself. That's fine because `HashMap` stores its values on the
// heap (a struct directly containing itself would have infinite size).
//
// YOUR TASK
//   Implement `add_entry` (recursive) and `parse_vss_from_str`.
//   Rules (same as the databroker):
//     * branch without `children`            -> VssError("children required for type branch")
//     * sensor/actuator/attribute without `datatype`
//                                              -> VssError("datatype required for <path>")
//     * child path = "<parent path>.<child name>"
//   Use `?` to propagate errors out of the recursion.
//   Do not edit the tests.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EntryType {
    Branch,
    Sensor,
    Actuator,
    Attribute,
}

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    entry_type: EntryType,
    description: String,
    children: Option<HashMap<String, Entry>>,
    #[serde(rename = "datatype")]
    data_type: Option<String>,
}

/// The whole file: a map from root name ("Vehicle") to its entry.
#[derive(Debug, Deserialize)]
struct RootEntry(HashMap<String, Entry>);

#[derive(Debug, PartialEq)]
struct DataEntry {
    entry_type: EntryType,
    data_type: String,
    description: String,
}

#[derive(Debug, PartialEq)]
struct VssError(String);

impl From<serde_json::Error> for VssError {
    fn from(err: serde_json::Error) -> Self {
        VssError(err.to_string())
    }
}

fn add_entry(
    entries: &mut BTreeMap<String, DataEntry>,
    path: String,
    entry: Entry,
) -> Result<(), VssError> {
    match entry.entry_type {
        EntryType::Branch => {
            let children = entry
                .children
                .ok_or_else(|| VssError("children required for type branch".to_string()))?;
            for (name, child) in children {
                add_entry(entries, format!("{path}.{name}"), child)?;
            }
        }
        entry_type @ (EntryType::Sensor | EntryType::Actuator | EntryType::Attribute) => {
            let Some(data_type) = entry.data_type else {
                return Err(VssError(format!("datatype required for {path}")));
            };
            entries.insert(
                path,
                DataEntry {
                    entry_type,
                    data_type,
                    description: entry.description,
                },
            );
        }
    }
    Ok(())
}

fn parse_vss_from_str(json: &str) -> Result<BTreeMap<String, DataEntry>, VssError> {
    let root: RootEntry = serde_json::from_str(json)?;
    let mut entries = BTreeMap::new();
    for (path, entry) in root.0 {
        add_entry(&mut entries, path, entry)?;
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VSS: &str = r#"{
      "Vehicle": {
        "type": "branch",
        "description": "High-level vehicle data.",
        "children": {
          "Speed": { "type": "sensor", "datatype": "float", "description": "Vehicle speed." },
          "Cabin": {
            "type": "branch",
            "description": "All in-cabin components.",
            "children": {
              "Door": {
                "type": "branch",
                "description": "Doors.",
                "children": {
                  "IsOpen": { "type": "actuator", "datatype": "boolean", "description": "Is door open?" }
                }
              }
            }
          },
          "VehicleIdentification": {
            "type": "branch",
            "description": "Attributes that identify a vehicle.",
            "children": {
              "VIN": { "type": "attribute", "datatype": "string", "description": "VIN." }
            }
          }
        }
      }
    }"#;

    #[test]
    fn flattens_to_dotted_paths() {
        let entries = parse_vss_from_str(VSS).expect("valid VSS");
        assert_eq!(entries.len(), 3, "branches themselves are not stored");
        assert_eq!(
            entries["Vehicle.Cabin.Door.IsOpen"],
            DataEntry {
                entry_type: EntryType::Actuator,
                data_type: "boolean".into(),
                description: "Is door open?".into(),
            }
        );
        assert_eq!(entries["Vehicle.Speed"].entry_type, EntryType::Sensor);
        assert_eq!(
            entries["Vehicle.VehicleIdentification.VIN"].data_type,
            "string"
        );
    }

    #[test]
    fn keys_come_out_sorted() {
        let entries = parse_vss_from_str(VSS).expect("valid VSS");
        let keys: Vec<&str> = entries.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "Vehicle.Cabin.Door.IsOpen",
                "Vehicle.Speed",
                "Vehicle.VehicleIdentification.VIN"
            ]
        );
    }

    #[test]
    fn branch_without_children_fails() {
        let json = r#"{ "Vehicle": { "type": "branch", "description": "x" } }"#;
        assert_eq!(
            parse_vss_from_str(json),
            Err(VssError("children required for type branch".into()))
        );
    }

    #[test]
    fn leaf_without_datatype_fails_with_its_path() {
        let json = r#"{ "Vehicle": { "type": "branch", "description": "x", "children": {
            "Speed": { "type": "sensor", "description": "no type" } } } }"#;
        assert_eq!(
            parse_vss_from_str(json),
            Err(VssError("datatype required for Vehicle.Speed".into()))
        );
    }

    #[test]
    fn invalid_json_is_reported() {
        let err = parse_vss_from_str("{ not json").unwrap_err();
        assert!(err.0.contains("key must be a string"), "got: {}", err.0);
    }
}
