// c04_e05_option_option_tristate.rs
//
// CONCEPT: `Option<Option<T>>` — three states: "don't touch", "clear it",
//          "set it to this".
// GUIDE:   chapters/04-enums/guide.md  (section "Option<Option<T>>: the tri-state update")
// DATABROKER: databroker/src/broker.rs — `EntryUpdate { actuator_target:
//             Option<Option<Datapoint>>, allowed: Option<Option<DataValue>>, .. }`
//             and `Entry::apply`, which only touches fields that are `Some(..)`.
// EXPECT:  tests fail: `todo!()` panics.
//
// An entry's `unit` is optional: `Option<String>`. An update to it needs to
// express three things:
//   None                    -> leave the unit as it is
//   Some(None)              -> remove the unit
//   Some(Some("km/h"))      -> set the unit to "km/h"
// A single `Option<String>` can't tell "leave it" from "remove it"; nesting
// can. The databroker's source even has a comment explaining this.
//
// YOUR TASK
//   Implement `apply`. Return the names of the fields that actually changed
//   (in the order "value", "unit"). Setting a field to the value it already
//   has is NOT a change. Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    value: Option<i32>,
    unit: Option<String>,
}

#[derive(Debug, Default)]
struct Update {
    value: Option<Option<i32>>,
    unit: Option<Option<String>>,
}

fn apply(entry: &mut Entry, update: Update) -> Vec<&'static str> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            value: Some(10),
            unit: Some("km/h".to_string()),
        }
    }

    #[test]
    fn empty_update_changes_nothing() {
        let mut e = entry();
        assert!(apply(&mut e, Update::default()).is_empty());
        assert_eq!(e, entry());
    }

    #[test]
    fn set_value() {
        let mut e = entry();
        let changed = apply(
            &mut e,
            Update {
                value: Some(Some(20)),
                ..Default::default()
            },
        );
        assert_eq!(changed, vec!["value"]);
        assert_eq!(e.value, Some(20));
        assert_eq!(e.unit, Some("km/h".to_string()));
    }

    #[test]
    fn clear_unit() {
        let mut e = entry();
        let changed = apply(
            &mut e,
            Update {
                unit: Some(None),
                ..Default::default()
            },
        );
        assert_eq!(changed, vec!["unit"]);
        assert_eq!(e.unit, None);
        assert_eq!(e.value, Some(10));
    }

    #[test]
    fn same_value_is_not_a_change() {
        let mut e = entry();
        let changed = apply(
            &mut e,
            Update {
                value: Some(Some(10)),
                unit: Some(Some("mph".to_string())),
            },
        );
        assert_eq!(changed, vec!["unit"]);
        assert_eq!(e.unit, Some("mph".to_string()));
    }

    #[test]
    fn clear_both() {
        let mut e = entry();
        let changed = apply(
            &mut e,
            Update {
                value: Some(None),
                unit: Some(None),
            },
        );
        assert_eq!(changed, vec!["value", "unit"]);
        assert_eq!(
            e,
            Entry {
                value: None,
                unit: None
            }
        );
    }
}
