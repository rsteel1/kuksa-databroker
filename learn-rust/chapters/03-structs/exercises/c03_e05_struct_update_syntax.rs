// c03_e05_struct_update_syntax.rs
//
// CONCEPT: struct update syntax `..Default::default()`, and `Option<T>` fields
//          meaning "not part of this update".
// GUIDE:   chapters/03-structs/guide.md  (section "Struct update syntax")
// DATABROKER: databroker/src/broker.rs — `pub struct EntryUpdate` has a dozen
//             `Option` fields and derives `Default`; callers write
//             `EntryUpdate { datapoint: Some(..), ..Default::default() }`.
// EXPECT:  compile error E0063 (missing fields in initializer).
//
// `Option<T>` is either `Some(value)` or `None`. (Chapter 04 covers it
// properly.) In an update struct, `None` means "leave this field alone".
// Struct update syntax fills in every field you don't mention from another
// value — usually `Default::default()`, which for `Option` is `None`.
//
// YOUR TASK
//   1. Fix `value_update` with `..Default::default()`.
//   2. Implement `describe_update` and `metadata_update` (use update syntax
//      in `metadata_update`, too).
//   Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Default, PartialEq)]
struct EntryUpdate {
    path: Option<String>,
    value: Option<i64>,
    description: Option<String>,
    unit: Option<String>,
    min: Option<i64>,
    max: Option<i64>,
}

/// An update that only changes the value.
fn value_update(value: i64) -> EntryUpdate {
    EntryUpdate { value: Some(value) }
}

/// An update that sets description and unit, and nothing else.
fn metadata_update(description: &str, unit: &str) -> EntryUpdate {
    todo!()
}

/// Counts how many fields the update would change.
fn describe_update(update: &EntryUpdate) -> usize {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_value_is_set() {
        let update = value_update(80);
        assert_eq!(update.value, Some(80));
        assert_eq!(update.path, None);
        assert_eq!(describe_update(&update), 1);
    }

    #[test]
    fn metadata_only() {
        let update = metadata_update("Vehicle speed", "km/h");
        assert_eq!(
            update,
            EntryUpdate {
                description: Some("Vehicle speed".to_string()),
                unit: Some("km/h".to_string()),
                path: None,
                value: None,
                min: None,
                max: None,
            }
        );
        assert_eq!(describe_update(&update), 2);
    }

    #[test]
    fn empty_update_changes_nothing() {
        assert_eq!(describe_update(&EntryUpdate::default()), 0);
    }
}
