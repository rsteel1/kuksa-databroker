// c28_e03_enum_representations.rs
//
// CONCEPT: deserializing enums — `rename_all`, per-variant `rename`, `untagged`.
// GUIDE:   chapters/28-serde-and-vss/guide.md  (section "Enums in serde")
// DATABROKER: databroker/src/vss.rs — `enum EntryType`, `enum ChangeType`, `enum DataType`
//             (every variant carries a `#[serde(rename = "...")]`)
// EXPECT:  tests fail at runtime: "unknown variant `sensor`, expected one of `Actuator`, ..."
//
// By default serde expects the JSON string to match the Rust variant name
// exactly ("Sensor"). VSS uses lowercase ("sensor", "onchange") and some
// names that could never be Rust identifiers ("string[]").
//
// The databroker writes `#[serde(rename = "...")]` on every single variant.
// A container attribute, `#[serde(rename_all = "lowercase")]`, does the same
// in one line — you'll use it here, and fall back to per-variant renames only
// where `rename_all` can't produce the right name.
//
// `min`/`max` in VSS can be a number, a bool or a string. An *untagged*
// enum tries each variant in order and picks the first that fits.
//
// YOUR TASK
//   1. `EntryType` and `ChangeType`: accept lowercase names.
//   2. `DataType`: scalars are lowercase ("boolean", "int8", ...); arrays are
//      "string[]" and "int32[]".
//   3. `Scalar`: make it untagged. Think about variant ORDER: 3 must become
//      `Int(3)`, not `Float(3.0)`.
//   Do not edit the tests.

use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EntryType {
    Actuator,
    Attribute,
    Branch,
    Sensor,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ChangeType {
    Static,
    OnChange,
    Continuous,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DataType {
    String,
    Boolean,
    Int8,
    Int32,
    Float,
    // WHY: `rename_all` would produce "stringarray"; the VSS name needs an explicit rename.
    #[serde(rename = "string[]")]
    StringArray,
    #[serde(rename = "int32[]")]
    Int32Array,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
enum Scalar {
    // WHY: untagged enums try variants top to bottom. `Int` must come before
    // `Float`, otherwise every integer would be accepted as a float first.
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn de<T: for<'de> Deserialize<'de>>(json: &str) -> T {
        serde_json::from_str(json).expect("should deserialize")
    }

    #[test]
    fn entry_types() {
        assert_eq!(de::<EntryType>(r#""sensor""#), EntryType::Sensor);
        assert_eq!(de::<EntryType>(r#""branch""#), EntryType::Branch);
        assert_eq!(de::<EntryType>(r#""actuator""#), EntryType::Actuator);
        assert_eq!(de::<EntryType>(r#""attribute""#), EntryType::Attribute);
    }

    #[test]
    fn change_types() {
        assert_eq!(de::<ChangeType>(r#""onchange""#), ChangeType::OnChange);
        assert_eq!(de::<ChangeType>(r#""continuous""#), ChangeType::Continuous);
        assert_eq!(de::<ChangeType>(r#""static""#), ChangeType::Static);
    }

    #[test]
    fn data_types() {
        assert_eq!(de::<DataType>(r#""boolean""#), DataType::Boolean);
        assert_eq!(de::<DataType>(r#""int8""#), DataType::Int8);
        assert_eq!(de::<DataType>(r#""float""#), DataType::Float);
        assert_eq!(de::<DataType>(r#""string""#), DataType::String);
        assert_eq!(de::<DataType>(r#""string[]""#), DataType::StringArray);
        assert_eq!(de::<DataType>(r#""int32[]""#), DataType::Int32Array);
    }

    #[test]
    fn unknown_variant_is_rejected() {
        let err = serde_json::from_str::<DataType>(r#""int128""#).expect_err("not a VSS type");
        assert!(err.to_string().contains("unknown variant `int128`"));
    }

    #[test]
    fn untagged_scalars_pick_the_first_fitting_variant() {
        assert_eq!(de::<Scalar>("3"), Scalar::Int(3));
        assert_eq!(de::<Scalar>("-40"), Scalar::Int(-40));
        assert_eq!(de::<Scalar>("2.5"), Scalar::Float(2.5));
        assert_eq!(de::<Scalar>("true"), Scalar::Bool(true));
        assert_eq!(de::<Scalar>(r#""km/h""#), Scalar::Text("km/h".to_string()));
    }
}
