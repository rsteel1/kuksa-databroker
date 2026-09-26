// c18_e05_reading_derive_output.rs
//
// CONCEPT: a `#[derive(...)]` is a procedural macro that writes ordinary
//          impl blocks for you. Writing them by hand once shows what the
//          derive generates, and where it falls short.
// GUIDE:   chapters/18-macros/guide.md  (section "What derive expands to")
// DATABROKER: databroker/src/broker.rs — `#[derive(Debug, Clone, PartialEq)]
//             pub struct Datapoint` (line 105); databroker/src/vss.rs —
//             `#[derive(Debug, Deserialize)]` on the VSS JSON types (line 39).
// EXPECT:  tests fail: the hand-written impls are `todo!()`.
//
// `mod derived` holds a `Datapoint` that uses `#[derive]`. The top-level
// `Datapoint` has the same fields, but you write its impls by hand. The tests
// compare the two, byte for byte, including `{:#?}` pretty-printing.
//
// YOUR TASK
//   Replace every `todo!()`:
//     - `Debug`, `Clone`, `PartialEq` and `Default` for `Datapoint`, producing
//       exactly what the derives in `mod derived` produce (hint: the
//       `Formatter::debug_struct` helper is what derive(Debug) calls);
//     - `Clone` for `Shared<T>` *without* requiring `T: Clone` (this is why it
//       can't be derived; see the guide);
//     - `Deserialize` for `SignalId`, rejecting negative numbers with the
//       error message "signal id must be >= 0, got <n>".
//   Do not edit `mod derived` or the tests.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::sync::Arc;

mod derived {
    #[derive(Debug, Clone, PartialEq, Default)]
    pub struct Datapoint {
        pub path: String,
        pub value: i64,
        pub source_ts: Option<u64>,
    }
}

struct Datapoint {
    path: String,
    value: i64,
    source_ts: Option<u64>,
}

impl fmt::Debug for Datapoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // WHY: `debug_struct` handles both `{:?}` and `{:#?}` (it checks
        // `f.alternate()` for you). That is exactly what the derive emits.
        f.debug_struct("Datapoint")
            .field("path", &self.path)
            .field("value", &self.value)
            .field("source_ts", &self.source_ts)
            .finish()
    }
}

impl Clone for Datapoint {
    fn clone(&self) -> Self {
        Datapoint {
            path: self.path.clone(),
            value: self.value,
            source_ts: self.source_ts,
        }
    }
}

impl PartialEq for Datapoint {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.value == other.value && self.source_ts == other.source_ts
    }
}

// WHY: clippy (rightly) says "this impl can be derived"; writing it by
// hand is the point of this exercise, so silence that one lint here only.
#[allow(clippy::derivable_impls)]
impl Default for Datapoint {
    fn default() -> Self {
        Datapoint {
            path: String::default(),
            value: i64::default(),
            source_ts: None,
        }
    }
}

/// A cheap-to-clone handle. `#[derive(Clone)]` would add a `T: Clone` bound
/// (derive bounds every type parameter), even though cloning an `Arc<T>`
/// never clones the `T`.
struct Shared<T> {
    inner: Arc<T>,
    version: u32,
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared {
            inner: Arc::clone(&self.inner),
            version: self.version,
        }
    }
}

/// A validated id: what `#[derive(Deserialize)]` can't express on its own.
#[derive(Debug, PartialEq)]
struct SignalId(i32);

impl<'de> Deserialize<'de> for SignalId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // WHY: reuse `i32`'s own Deserialize impl, then validate. `custom`
        // comes from the `serde::de::Error` trait imported above.
        let raw = i32::deserialize(deserializer)?;
        if raw < 0 {
            return Err(D::Error::custom(format!(
                "signal id must be >= 0, got {raw}"
            )));
        }
        Ok(SignalId(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Datapoint, derived::Datapoint) {
        (
            Datapoint {
                path: "Vehicle.Speed".into(),
                value: 42,
                source_ts: Some(1_700_000_000),
            },
            derived::Datapoint {
                path: "Vehicle.Speed".into(),
                value: 42,
                source_ts: Some(1_700_000_000),
            },
        )
    }

    #[test]
    fn debug_matches_derive() {
        let (manual, derived) = pair();
        assert_eq!(format!("{manual:?}"), format!("{derived:?}"));
    }

    #[test]
    fn pretty_debug_matches_derive() {
        let (manual, derived) = pair();
        assert_eq!(format!("{manual:#?}"), format!("{derived:#?}"));
    }

    #[test]
    fn clone_and_eq() {
        let (manual, _) = pair();
        let copy = manual.clone();
        assert!(copy == manual);
        let mut other = manual.clone();
        other.source_ts = None;
        assert!(other != manual);
    }

    #[test]
    fn default_matches_derive() {
        let manual = Datapoint::default();
        let derived = derived::Datapoint::default();
        assert_eq!(format!("{manual:?}"), format!("{derived:?}"));
    }

    #[test]
    fn shared_clones_without_cloning_the_inner_value() {
        struct NotClone; // deliberately does NOT implement Clone
        let a = Shared {
            inner: Arc::new(NotClone),
            version: 3,
        };
        let b = a.clone();
        assert!(Arc::ptr_eq(&a.inner, &b.inner));
        assert_eq!(b.version, 3);
        assert_eq!(Arc::strong_count(&a.inner), 2);
    }

    #[test]
    fn signal_id_deserializes_and_validates() {
        assert_eq!(serde_json::from_str::<SignalId>("7").unwrap(), SignalId(7));
        let err = serde_json::from_str::<SignalId>("-1").unwrap_err();
        assert!(
            err.to_string().contains("signal id must be >= 0, got -1"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn manual_impl_composes_with_derived_ones() {
        #[derive(Debug, Deserialize)]
        struct Request {
            ids: Vec<SignalId>,
        }
        let ok: Request = serde_json::from_str(r#"{"ids": [1, 2, 3]}"#).unwrap();
        assert_eq!(ok.ids, vec![SignalId(1), SignalId(2), SignalId(3)]);
        assert!(serde_json::from_str::<Request>(r#"{"ids": [1, -5]}"#).is_err());
    }
}
