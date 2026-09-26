// c14_e03_orphan_rule_newtype.rs
//
// CONCEPT: the orphan rule — you may implement a trait for a type only if
//          the trait or the type is defined in your crate — and the newtype
//          workaround.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "The orphan rule and the newtype pattern")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs — every
//             `impl From<X> for Y` there has one side that is local to the
//             crate (`broker::DataValue`, `broker::Datapoint`, ...). The protobuf
//             types live in another crate (`databroker-proto`), so conversions
//             *between two proto types* could not be written in `databroker`.
// EXPECT:  compile error E0117 (only traits defined in the current crate can
//          be implemented for types defined outside of the crate)
//
// `Display` is std's trait and `Vec` is std's type, so `impl Display for
// Vec<Reading>` would let two crates write conflicting impls for the same
// thing — Rust forbids it. Wrapping the Vec in YOUR struct makes the type
// local, and then any trait impl is allowed.
//
// YOUR TASK
//   Implement `Display` for the newtype `Readings` instead of `Vec<Reading>`.
//   Output: "Speed=10, Rpm=1800" (", " between entries, "" when empty).
//   Also implement `From<Vec<Reading>> for Readings` so `.into()` works.
//   Do not edit the tests.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
struct Reading {
    name: &'static str,
    value: i64,
}

/// A local wrapper: zero runtime cost, but now the type is ours.
struct Readings(Vec<Reading>);

impl fmt::Display for Readings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, reading) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}={}", reading.name, reading.value)?;
        }
        Ok(())
    }
}

impl From<Vec<Reading>> for Readings {
    fn from(readings: Vec<Reading>) -> Self {
        Readings(readings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &'static str, value: i64) -> Reading {
        Reading { name, value }
    }

    #[test]
    fn displays_through_the_newtype() {
        let readings = Readings(vec![r("Speed", 10), r("Rpm", 1800)]);
        assert_eq!(readings.to_string(), "Speed=10, Rpm=1800");
        assert_eq!(Readings(vec![]).to_string(), "");
    }

    #[test]
    fn into_newtype() {
        let readings: Readings = vec![r("Speed", 1)].into();
        assert_eq!(readings.0, vec![r("Speed", 1)]);
    }
}
