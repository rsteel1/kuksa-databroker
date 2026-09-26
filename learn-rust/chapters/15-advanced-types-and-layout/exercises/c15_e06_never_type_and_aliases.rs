// c15_e06_never_type_and_aliases.rs
//
// CONCEPT: the never type `!`, `Infallible`, and type aliases for long types
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "The never type and type aliases")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `type SubscribeStream = Pin<Box<dyn
//             Stream<...> + Send + Sync + 'static>>` names a long type once;
//             `ActuationSubscriptionSnapshot` in broker.rs is a tuple alias.
// EXPECT:  tests fail: `todo!()` panics
//
// `!` ("never") is the type of expressions that never produce a value:
// `return`, `break`, `continue`, `panic!()`, `loop {}`. Because it never
// produces a value it can stand in for *any* type, which is why
// `let x: u32 = match o { Some(v) => v, None => return Err(..) };` compiles.
//
// `std::convert::Infallible` is an enum with no variants: a `Result<T,
// Infallible>` can only ever be `Ok`. You get one for free whenever a
// `From` impl exists: `TryFrom` is implemented with `Error = Infallible`.
//
// `type` gives a new *name* to an existing type (not a new type!).
//
// YOUR TASK
//   Implement the `todo!()`s. `into_ok` must not use `unwrap`/`expect`/panic.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::BTreeMap;
use std::convert::Infallible;

#[derive(Debug, PartialEq)]
enum ParseError {
    Empty,
    NotANumber(String),
}

/// An alias: `ParseResult<T>` is exactly `Result<T, ParseError>`.
type ParseResult<T> = Result<T, ParseError>;

/// A boxed iterator over (path, value) pairs, borrowing from `'a`.
/// `+ 'a` says the iterator may hold references that live for `'a` (chapter 16).
type UpdateIter<'a> = Box<dyn Iterator<Item = (&'a str, i64)> + 'a>;

fn parse_one(input: &str) -> ParseResult<u32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }
    trimmed
        .parse()
        .map_err(|_| ParseError::NotANumber(trimmed.to_string()))
}

/// Parses every input, stopping at the first error.
fn parse_all(inputs: &[&str]) -> ParseResult<Vec<u32>> {
    todo!()
}

/// Extracts the value from a `Result` that cannot be `Err`, with no panic.
fn into_ok<T>(result: Result<T, Infallible>) -> T {
    todo!()
}

struct Store {
    values: BTreeMap<String, i64>,
}

impl Store {
    /// Iterates over entries whose value is strictly above `threshold`.
    fn updates_above(&self, threshold: i64) -> UpdateIter<'_> {
        todo!()
    }

    /// Returns the value or bails out early from the `match` with `return`.
    fn doubled(&self, path: &str) -> ParseResult<i64> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_is_the_same_type() {
        let r: Result<Vec<u32>, ParseError> = parse_all(&["1", " 2 ", "30"]);
        assert_eq!(r, Ok(vec![1, 2, 30]));
        assert_eq!(
            parse_all(&["1", "x", ""]),
            Err(ParseError::NotANumber("x".into()))
        );
        assert_eq!(parse_all(&["", "x"]), Err(ParseError::Empty));
    }

    #[test]
    fn infallible_conversions() {
        // Parsing a `String` from a `&str` can't fail: `<String as FromStr>::Err`
        // is `Infallible`. (Likewise `TryFrom` is `Infallible` wherever `From` exists.)
        let parsed: Result<String, Infallible> = "Vehicle.Speed".parse::<String>();
        assert_eq!(into_ok(parsed), "Vehicle.Speed");
        assert_eq!(into_ok::<&str>(Ok("fine")), "fine");
    }

    #[test]
    fn boxed_iterator_alias() {
        let store = Store {
            values: BTreeMap::from([
                ("Vehicle.Speed".to_string(), 120),
                ("Vehicle.Width".to_string(), 2),
                ("Vehicle.Height".to_string(), 150),
            ]),
        };
        let above: Vec<_> = store.updates_above(100).collect();
        assert_eq!(above, [("Vehicle.Height", 150), ("Vehicle.Speed", 120)]);
        assert_eq!(store.doubled("Vehicle.Width"), Ok(4));
        assert_eq!(store.doubled("missing"), Err(ParseError::Empty));
    }
}
