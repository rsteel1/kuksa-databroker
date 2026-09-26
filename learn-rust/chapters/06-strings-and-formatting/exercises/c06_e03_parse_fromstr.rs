// c06_e03_parse_fromstr.rs
//
// CONCEPT: `str::parse::<T>()` and implementing `FromStr` so your own type
//          can be parsed the same way.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Parsing text: parse and FromStr")
// DATABROKER: databroker-proto/src/lib.rs — `impl FromStr for DataType`
//             ("int8[]" -> DataType::Int8Array) and `Value::new` parses values
//             with `value.parse::<i8>()`; the CLI uses both to turn what you
//             type into protobuf values.
// EXPECT:  tests fail: `todo!()` panics
//
// `"42".parse::<u8>()` works because `u8` implements the standard `FromStr`
// interface. Implement `FromStr` for your own type and `"...".parse::<Update>()`
// works too. Parsing can fail, so it returns `Result<T, E>`: like `Option`,
// but the failure case carries a reason (`Err(e)`). Chapter 07 goes deep on
// `Result`; here you only need `Ok(..)`, `Err(..)` and `match`.
//
// YOUR TASK
//   Implement `parse_percent` and `Update::from_str`. Rules for an update:
//   - format is "<path>=<number>", whitespace around either part is ignored;
//   - no '=' at all          -> Err(ParseUpdateError::MissingEquals)
//   - empty path             -> Err(ParseUpdateError::EmptyPath)
//   - number doesn't parse   -> Err(ParseUpdateError::BadNumber(<the trimmed text>))
//   Use `split_once`, `trim` and `parse::<f64>()`.
//   Do not edit the tests.

// I AM NOT DONE

use std::str::FromStr;

/// "0".."100" -> Some(n); anything else (including "101", "-1", "abc") -> None.
fn parse_percent(s: &str) -> Option<u8> {
    todo!()
}

#[derive(Debug, PartialEq)]
struct Update {
    path: String,
    value: f64,
}

#[derive(Debug, PartialEq)]
enum ParseUpdateError {
    MissingEquals,
    EmptyPath,
    BadNumber(String),
}

// An `impl FromStr for Update` block fills in the standard "parse me from a
// &str" interface. `type Err` names the error type this parser returns.
impl FromStr for Update {
    type Err = ParseUpdateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent() {
        assert_eq!(parse_percent("0"), Some(0));
        assert_eq!(parse_percent(" 85 "), Some(85));
        assert_eq!(parse_percent("100"), Some(100));
        assert_eq!(parse_percent("101"), None);
        assert_eq!(parse_percent("-1"), None);
        assert_eq!(parse_percent("abc"), None);
    }

    #[test]
    fn parses_valid_updates() {
        assert_eq!(
            "Vehicle.Speed=42.5".parse::<Update>(),
            Ok(Update {
                path: "Vehicle.Speed".to_string(),
                value: 42.5
            })
        );
        assert_eq!(
            "  Vehicle.Rpm =  1800 ".parse::<Update>(),
            Ok(Update {
                path: "Vehicle.Rpm".to_string(),
                value: 1800.0
            })
        );
    }

    #[test]
    fn reports_why_parsing_failed() {
        assert_eq!(
            "Vehicle.Speed".parse::<Update>(),
            Err(ParseUpdateError::MissingEquals)
        );
        assert_eq!(" = 3".parse::<Update>(), Err(ParseUpdateError::EmptyPath));
        assert_eq!(
            "Vehicle.Speed= fast ".parse::<Update>(),
            Err(ParseUpdateError::BadNumber("fast".to_string()))
        );
    }
}
