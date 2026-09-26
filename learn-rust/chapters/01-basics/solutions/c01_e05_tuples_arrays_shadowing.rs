// c01_e05_tuples_arrays_shadowing.rs
//
// CONCEPT: tuples, fixed-size arrays, destructuring, shadowing.
// GUIDE:   chapters/01-basics/guide.md  (section "Tuples, arrays and shadowing")
// DATABROKER: databroker/src/main.rs — the port is read as text from the
//             command line and turned into a `u16` (`value_parser!(u16)`).
// EXPECT:  compile errors E0308 (mismatched types) and E0277 (can't compare
//          `&str` with an integer) in `parse_port`.
//
// *Shadowing* means declaring a new variable with the same name as an old
// one: `let x = ...; let x = transform(x);`. The new `x` may even have a
// different type. It's the idiomatic way to express "the same value, in a
// new form" (text -> trimmed text -> number) without inventing names like
// `port_str`, `port_trimmed`, `port_num`.
//
// YOUR TASK
//   1. Fix `parse_port` using shadowing (`let port = ...` again), not `mut`.
//   2. Implement `min_max` and return a tuple `(min, max)`.
//   3. Implement `swap` using tuple destructuring.
//   Do not edit the tests.

/// Parses a port given as text (maybe with surrounding spaces).
fn parse_port(input: &str) -> u16 {
    let port = input.trim();
    // WHY: shadowing — a *new* binding named `port`, with a new type (u16).
    let port: u16 = port.parse().expect("tests only pass valid ports");
    assert!(port != 0, "port 0 is not a usable port");
    port
}

/// Smallest and largest of exactly five readings.
fn min_max(readings: [i32; 5]) -> (i32, i32) {
    let mut min = readings[0];
    let mut max = readings[0];
    for reading in readings {
        if reading < min {
            min = reading;
        }
        if reading > max {
            max = reading;
        }
    }
    (min, max)
}

/// Swaps the two halves of a pair.
fn swap(pair: (String, u32)) -> (u32, String) {
    let (name, id) = pair;
    (id, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_parsing() {
        assert_eq!(parse_port("55555"), 55555);
        assert_eq!(parse_port("  8080 \n"), 8080);
    }

    #[test]
    fn extremes() {
        assert_eq!(min_max([3, -7, 12, 0, 5]), (-7, 12));
        assert_eq!(min_max([1, 1, 1, 1, 1]), (1, 1));
        let (lo, hi) = min_max([100, 90, 80, 70, 60]);
        assert_eq!(lo, 60);
        assert_eq!(hi, 100);
    }

    #[test]
    fn swapping() {
        assert_eq!(
            swap(("Vehicle.Speed".to_string(), 7)),
            (7, "Vehicle.Speed".to_string())
        );
    }
}
