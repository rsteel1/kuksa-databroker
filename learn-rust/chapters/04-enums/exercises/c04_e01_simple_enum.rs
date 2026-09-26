// c04_e01_simple_enum.rs
//
// CONCEPT: enums as "one of a fixed set of choices", and `match`.
// GUIDE:   chapters/04-enums/guide.md  (section "Enums: one of several variants")
// DATABROKER: databroker/src/types.rs — `pub enum EntryType { Sensor,
//             Attribute, Actuator }` and `pub enum ChangeType { Static,
//             OnChange, Continuous }`; databroker/src/vss.rs —
//             `determine_change_type`.
// EXPECT:  compile error E0599 (no variant named `Actuator`).
//
// In VSS every signal is one of three kinds:
//   Sensor    — measured by the car (speed, temperature)
//   Attribute — fixed facts (vehicle width, number of doors)
//   Actuator  — something you can command (door lock, window position)
// An enum makes "any other kind" impossible to express.
//
// YOUR TASK
//   1. Add the missing variant to `EntryType`.
//   2. Implement `can_actuate`, `default_change_type` and `from_name` with `match`.
//   Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryType {
    Sensor,
    Attribute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeType {
    Static,
    OnChange,
    Continuous,
}

/// Only actuators accept actuation requests.
fn can_actuate(entry_type: EntryType) -> bool {
    todo!()
}

/// The change type used when the VSS file doesn't specify one: attributes
/// never change (`Static`); sensors and actuators default to `Continuous`.
/// This is exactly the fallback in `vss.rs` `determine_change_type`.
fn default_change_type(entry_type: EntryType) -> ChangeType {
    todo!()
}

/// Parses the VSS JSON spelling ("sensor", "attribute", "actuator").
fn from_name(name: &str) -> Option<EntryType> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actuation() {
        assert!(can_actuate(EntryType::Actuator));
        assert!(!can_actuate(EntryType::Sensor));
        assert!(!can_actuate(EntryType::Attribute));
    }

    #[test]
    fn change_types() {
        assert_eq!(
            default_change_type(EntryType::Attribute),
            ChangeType::Static
        );
        assert_eq!(
            default_change_type(EntryType::Sensor),
            ChangeType::Continuous
        );
        assert_eq!(
            default_change_type(EntryType::Actuator),
            ChangeType::Continuous
        );
        assert_ne!(ChangeType::Continuous, ChangeType::OnChange);
    }

    #[test]
    fn names() {
        assert_eq!(from_name("actuator"), Some(EntryType::Actuator));
        assert_eq!(from_name("sensor"), Some(EntryType::Sensor));
        assert_eq!(from_name("attribute"), Some(EntryType::Attribute));
        assert_eq!(from_name("branch"), None);
    }
}
