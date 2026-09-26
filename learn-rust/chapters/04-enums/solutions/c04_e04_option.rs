// c04_e04_option.rs
//
// CONCEPT: `Option<T>` — `if let`, `let ... else`, `?` on Option, and the
//          handy combinators `map` / `unwrap_or`.
// GUIDE:   chapters/04-enums/guide.md  (section "Option: a value or nothing")
// DATABROKER: databroker/src/broker.rs — `get_metadata_by_path`:
//             `let id = self.db.path_to_id.get(path)?;` and
//             `Metadata { unit: Option<String>, min: Option<DataValue>, .. }`.
// EXPECT:  tests fail: `todo!()` panics.
//
// Rust has no null. "Maybe there is a value" is spelled `Option<T>`, an enum
// with `Some(T)` and `None`. The compiler won't let you use the `T` until you
// have handled the `None` case.
//
// YOUR TASK
//   Implement the four functions. Each doc comment names the tool to use.
//   Do not edit the tests.

/// Looks up the id registered for `path`. Use a `for` loop over the pairs
/// and `return Some(..)`; `None` if not found.
fn find_id(registry: &[(&str, i32)], path: &str) -> Option<i32> {
    for &(registered, id) in registry {
        if registered == path {
            return Some(id);
        }
    }
    None
}

/// The unit to display: the unit if present, otherwise "" . Use `unwrap_or`.
fn display_unit(unit: Option<&str>) -> &str {
    unit.unwrap_or("")
}

/// Length of the leaf segment of `path` (after the last '.'), or `None` if
/// the path has no '.'. Use `?` on the `Option` returned by `rsplit_once`.
fn leaf_len(path: &str) -> Option<usize> {
    let (_, leaf) = path.rsplit_once('.')?;
    Some(leaf.len())
}

/// Parses an interval like "100ms" into 100. Anything else -> `None`.
/// Use `?` for the suffix (it's an Option), and `let Ok(..) = ... else`
/// for the number: `parse` returns a `Result`, and `?` can't turn a Result
/// error into `None` by itself.
fn parse_interval_ms(text: &str) -> Option<u32> {
    let number = text.strip_suffix("ms")?;
    let Ok(ms) = number.parse::<u32>() else {
        return None;
    };
    Some(ms)
}

/// Describes an optional min/max bound, e.g. Some(10) -> "min=10",
/// None -> "min=unbounded". Use `if let` or `match`.
fn describe_min(min: Option<i32>) -> String {
    if let Some(value) = min {
        format!("min={value}")
    } else {
        String::from("min=unbounded")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        let registry = [("Vehicle.Speed", 1), ("Vehicle.Width", 2)];
        assert_eq!(find_id(&registry, "Vehicle.Width"), Some(2));
        assert_eq!(find_id(&registry, "Vehicle.Height"), None);
    }

    #[test]
    fn units() {
        assert_eq!(display_unit(Some("km/h")), "km/h");
        assert_eq!(display_unit(None), "");
    }

    #[test]
    fn leaves() {
        assert_eq!(leaf_len("Vehicle.Speed"), Some(5));
        assert_eq!(leaf_len("Vehicle"), None);
    }

    #[test]
    fn intervals() {
        assert_eq!(parse_interval_ms("100ms"), Some(100));
        assert_eq!(parse_interval_ms("100"), None);
        assert_eq!(parse_interval_ms("fastms"), None);
        assert_eq!(parse_interval_ms("-5ms"), None);
    }

    #[test]
    fn bounds() {
        assert_eq!(describe_min(Some(10)), "min=10");
        assert_eq!(describe_min(None), "min=unbounded");
    }
}
