// c01_e03_functions.rs
//
// CONCEPT: functions, parameters, return types, expressions vs statements.
// GUIDE:   chapters/01-basics/guide.md  (section "Functions, statements and expressions")
// DATABROKER: databroker/src/types.rs — `SignalId::id` and `TimeInterval::new`
//             are one-line functions whose body is a single expression.
// EXPECT:  compile error E0308 (mismatched types: expected `f64`, found `()`).
//
// In Rust almost everything is an *expression* that produces a value. A block
// `{ ... }` evaluates to its last expression — unless that expression ends in
// a semicolon, which turns it into a *statement* that produces `()` ("unit",
// Rust's "nothing").
//
// YOUR TASK
//   Fix `celsius_to_fahrenheit` (look closely at the last line) and implement
//   `is_valid_port` and `ms_to_seconds`. Do not edit the tests.

/// Converts a temperature reading, e.g. for `Vehicle.Cabin.Temperature`.
fn celsius_to_fahrenheit(celsius: f64) -> f64 {
    celsius * 9.0 / 5.0 + 32.0
}

/// The databroker listens on a TCP port. Port 0 is not a usable port and the
/// maximum is 65535.
fn is_valid_port(port: u32) -> bool {
    (1..=65535).contains(&port)
}

/// Subscriptions have a sample interval in milliseconds. Convert it to seconds.
fn ms_to_seconds(ms: u32) -> f64 {
    f64::from(ms) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freezing_and_boiling() {
        assert_eq!(celsius_to_fahrenheit(0.0), 32.0);
        assert_eq!(celsius_to_fahrenheit(100.0), 212.0);
    }

    #[test]
    fn ports() {
        assert!(is_valid_port(55555));
        assert!(is_valid_port(1));
        assert!(is_valid_port(65535)); // the top of the range is valid too
        assert!(!is_valid_port(0));
        assert!(!is_valid_port(65536));
        assert!(!is_valid_port(70000));
    }

    #[test]
    fn intervals() {
        assert_eq!(ms_to_seconds(1500), 1.5);
        assert_eq!(ms_to_seconds(0), 0.0);
    }
}
