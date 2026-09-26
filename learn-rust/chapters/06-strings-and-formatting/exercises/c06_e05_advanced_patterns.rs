// c06_e05_advanced_patterns.rs
//
// CONCEPT: patterns beyond the basics — slice patterns, `@` bindings,
//          guards, nested destructuring, binding modes (match ergonomics)
//          and `matches!`.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Advanced patterns")
// DATABROKER: databroker/src/broker.rs — `Entry::validate` / `DatabaseWriteAccess::update`
//             match on tuples like `(Some(_), Err(PermissionError::Denied))`;
//             databroker/src/authorization/jwt/decoder.rs `make_validator`
//             uses `a @ (Algorithm::RS256 | ...)`.
// EXPECT:  tests fail: `todo!()` panics
//
// A pattern can look *inside* a value: into slices (`[first, .., last]`),
// through `Option`s and enum variants, and bind parts with `name @ pattern`.
// When you match on a reference, Rust automatically binds the inner parts as
// references too ("match ergonomics") — which is how you can mutate through
// `&mut` without writing `ref mut`.
//
// YOUR TASK
//   Implement the five functions using `match` / `if let` / `matches!`
//   patterns (no indexing like `parts[0]`, no `.len()` comparisons except in
//   a guard). Do not edit the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    NotAvailable,
}

#[derive(Debug)]
struct Update {
    path: String,
    // Some(None) = "clear the target", Some(Some(v)) = "set it to v"
    target: Option<Option<Value>>,
}

/// Classify a path already split on '.':
///   []                          -> "empty"
///   ["Vehicle"]                 -> "root"
///   ["Vehicle", .., "IsOpen"]   -> "status"   (at least 2 parts)
///   ["Vehicle", more than 3 further parts...] -> "deep"
///   ["Vehicle", ..]             -> "vehicle"
///   anything else               -> "foreign"
fn classify(parts: &[&str]) -> &'static str {
    todo!()
}

/// 0..=30 -> "slow (n)", 31..=130 -> "normal (n)", anything above -> "fast (n)"
fn speed_band(kmh: u32) -> String {
    todo!()
}

/// Is this a number (Int or Float)? One `matches!` expression.
fn is_numeric(value: &Value) -> bool {
    todo!()
}

/// Add one to every Int in place; leave everything else alone.
fn bump_ints(values: &mut [Value]) {
    todo!()
}

/// The integer target of an update, if it sets one ("set to Int(t)").
/// Clearing (`Some(None)`), leaving unchanged (`None`) or a non-Int target -> None.
fn int_target(update: &Update) -> Option<i64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_paths() {
        assert_eq!(classify(&[]), "empty");
        assert_eq!(classify(&["Vehicle"]), "root");
        assert_eq!(
            classify(&["Vehicle", "Cabin", "Door", "Row1", "IsOpen"]),
            "status"
        );
        assert_eq!(classify(&["Vehicle", "IsOpen"]), "status");
        assert_eq!(classify(&["Vehicle", "A", "B", "C", "D"]), "deep");
        assert_eq!(classify(&["Vehicle", "Speed"]), "vehicle");
        assert_eq!(classify(&["Truck", "Speed"]), "foreign");
    }

    #[test]
    fn bands() {
        assert_eq!(speed_band(0), "slow (0)");
        assert_eq!(speed_band(30), "slow (30)");
        assert_eq!(speed_band(31), "normal (31)");
        assert_eq!(speed_band(130), "normal (130)");
        assert_eq!(speed_band(250), "fast (250)");
    }

    #[test]
    fn numeric() {
        assert!(is_numeric(&Value::Int(1)));
        assert!(is_numeric(&Value::Float(1.5)));
        assert!(!is_numeric(&Value::Text("1".into())));
        assert!(!is_numeric(&Value::NotAvailable));
    }

    #[test]
    fn bump() {
        let mut values = vec![Value::Int(1), Value::Text("x".into()), Value::Int(-1)];
        bump_ints(&mut values);
        assert_eq!(
            values,
            vec![Value::Int(2), Value::Text("x".into()), Value::Int(0)]
        );
    }

    #[test]
    fn targets() {
        let set = |target| Update {
            path: "Vehicle.Cabin.Seat.Position".into(),
            target,
        };
        assert_eq!(int_target(&set(Some(Some(Value::Int(40))))), Some(40));
        assert_eq!(int_target(&set(Some(Some(Value::Float(1.0))))), None);
        assert_eq!(int_target(&set(Some(None))), None);
        assert_eq!(int_target(&set(None)), None);
    }
}
