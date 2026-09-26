// c34_e04_read_serde_json_value.rs
//
// CONCEPT: read-then-rebuild `serde_json::Value`: an enum for "any JSON",
//          indexing that never panics on reads (`v["a"]["b"]` returns `Null`
//          when missing), a *sealed* helper trait so one `get()` accepts both
//          `usize` and `&str`, and JSON Pointer lookups.
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Reading trait-heavy and generic-heavy code")
// DATABROKER: databroker/src/vss.rs — VSS `default`/`allowed` values arrive as
//             `serde_json::Value` and are converted to typed `DataValue`s.
// EXPECT:  tests fail: `todo!()` panics
//
// READ FIRST (serde_json 1.0.151, under ~/.cargo/registry/src/index.crates.io-*/serde_json-1.0.151/):
//   src/value/mod.rs:116    `pub enum Value` — Null, Bool, Number, String, Array, Object
//   src/value/mod.rs:305    `pub fn get<I: Index>(&self, index: I)`
//   src/value/index.rs:37   `pub trait Index: private::Sealed` — `index_into`
//   src/value/index.rs:54   `impl Index for usize` and :83 `impl Index for str`
//   src/value/index.rs:212  `ops::Index::index` — `static NULL` for missing keys
//   src/value/mod.rs:779    `pub fn pointer` — "/a/0/b", with `~1` and `~0` escapes
//
// YOUR TASK
//   Implement the `todo!()`s. The last test parses documents with real
//   serde_json, converts them, and checks every lookup gives the same
//   answer both ways. Do not edit the tests.

// I AM NOT DONE

use std::collections::BTreeMap;
use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

mod private {
    /// Only this module can implement `Sealed`, so only the impls below can
    /// ever implement `ValueIndex`: the crate can add methods to it later
    /// without breaking anyone (see guide 33 on semver).
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl<T: ?Sized + Sealed> Sealed for &T {}
}

/// Something that can index into a `Value`: array positions or object keys.
pub trait ValueIndex: private::Sealed {
    /// `None` when the index doesn't apply (wrong variant, missing key, out
    /// of bounds).
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value>;
}

impl ValueIndex for usize {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        todo!()
    }
}

impl ValueIndex for str {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        todo!()
    }
}

impl ValueIndex for String {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        self.as_str().index_into(v)
    }
}

impl<T: ?Sized + ValueIndex> ValueIndex for &T {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).index_into(v)
    }
}

impl Value {
    pub fn get<I: ValueIndex>(&self, index: I) -> Option<&Value> {
        todo!()
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// RFC 6901 JSON Pointer: `""` is the whole document; otherwise the
    /// pointer must start with `/`, and each segment is unescaped
    /// (`~1` → `/`, then `~0` → `~`) and used as an object key or array index.
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        todo!()
    }
}

/// Reading a missing key or index gives `Null` instead of panicking, so
/// `v["a"]["b"]["c"]` is always safe to write.
impl<I: ValueIndex> ops::Index<I> for Value {
    type Output = Value;

    fn index(&self, index: I) -> &Value {
        todo!()
    }
}

/// Converts real serde_json values (numbers become f64).
impl From<&serde_json::Value> for Value {
    fn from(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(items) => {
                Value::Array(items.iter().map(Value::from).collect())
            }
            serde_json::Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Value::from(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VSS: &str = r#"{
        "Vehicle": {
            "type": "branch",
            "children": {
                "Speed": { "type": "sensor", "datatype": "float", "unit": "km/h", "max": 250 },
                "Cabin": {
                    "type": "branch",
                    "children": {
                        "Door": { "type": "branch", "instances": ["Row1", "Row2"] }
                    }
                },
                "a/b": { "type": "attribute", "default": null },
                "m~n": { "type": "attribute", "allowed": ["ECO", "SPORT"] }
            }
        }
    }"#;

    fn doc() -> Value {
        let real: serde_json::Value = serde_json::from_str(VSS).expect("valid JSON");
        Value::from(&real)
    }

    #[test]
    fn get_with_keys_and_positions() {
        let v = doc();
        let speed = v
            .get("Vehicle")
            .and_then(|b| b.get("children"))
            .and_then(|c| c.get("Speed"))
            .expect("speed exists");
        assert_eq!(speed.get("unit").and_then(Value::as_str), Some("km/h"));
        assert_eq!(speed.get("max").and_then(Value::as_f64), Some(250.0));
        let key = String::from("type");
        assert_eq!(speed.get(&key).and_then(Value::as_str), Some("sensor"));
        assert_eq!(speed.get(0), None, "an object can't be indexed by position");
        assert_eq!(speed.get("nope"), None);
    }

    #[test]
    fn index_operator_returns_null_for_missing_paths() {
        let v = doc();
        let instances = &v["Vehicle"]["children"]["Cabin"]["children"]["Door"]["instances"];
        assert_eq!(instances[1], Value::String("Row2".to_string()));
        assert!(instances[9].is_null());
        assert!(
            v["Nope"]["deeper"][3]["still"].is_null(),
            "never panics on reads"
        );
    }

    #[test]
    fn json_pointer() {
        let v = doc();
        assert_eq!(v.pointer(""), Some(&v));
        assert_eq!(
            v.pointer("/Vehicle/children/Speed/unit")
                .and_then(Value::as_str),
            Some("km/h")
        );
        assert_eq!(
            v.pointer("/Vehicle/children/Cabin/children/Door/instances/0")
                .and_then(Value::as_str),
            Some("Row1")
        );
        assert_eq!(
            v.pointer("/Vehicle/children/a~1b/default"),
            Some(&Value::Null),
            "~1 means /"
        );
        assert_eq!(
            v.pointer("/Vehicle/children/m~0n/allowed/1")
                .and_then(Value::as_str),
            Some("SPORT"),
            "~0 means ~"
        );
        assert_eq!(v.pointer("Vehicle"), None, "must start with /");
        assert_eq!(v.pointer("/Vehicle/children/Speed/unit/0"), None);
    }

    #[test]
    fn same_answers_as_real_serde_json() {
        let real: serde_json::Value = serde_json::from_str(VSS).expect("valid JSON");
        let mine = Value::from(&real);
        let pointers = [
            "",
            "/Vehicle",
            "/Vehicle/type",
            "/Vehicle/children/Speed/max",
            "/Vehicle/children/Cabin/children/Door/instances/1",
            "/Vehicle/children/Cabin/children/Door/instances/2",
            "/Vehicle/children/a~1b",
            "/Vehicle/children/a~1b/default",
            "/Vehicle/children/m~0n/allowed/0",
            "/Vehicle/children/m~01",
            "/Vehicle//x",
            "no-slash",
            "/Vehicle/children/Speed/unit/0",
        ];
        for pointer in pointers {
            let expected = real.pointer(pointer).map(Value::from);
            assert_eq!(
                mine.pointer(pointer).cloned(),
                expected,
                "pointer {pointer:?}"
            );
        }
        assert_eq!(
            Value::from(&real["Vehicle"]["x"]["y"]),
            mine["Vehicle"]["x"]["y"]
        );
    }
}
