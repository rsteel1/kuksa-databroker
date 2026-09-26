// c03_e01_define_struct.rs
//
// CONCEPT: declaring a struct, constructing it, reading and updating fields.
// GUIDE:   chapters/03-structs/guide.md  (section "Defining and building structs")
// DATABROKER: databroker/src/broker.rs — `pub struct Datapoint { ts, source_ts, value }`
// EXPECT:  compile error E0422 (cannot find struct `Datapoint`), plus
//          follow-on errors that disappear once the struct exists.
//
// A struct groups named fields into one type. The databroker's `Datapoint`
// is "a value plus when it was measured". We'll use a simplified version with
// plain numbers instead of `SystemTime` and `DataValue`.
//
// YOUR TASK
//   Declare `struct Datapoint` with these fields so the code below compiles:
//     path: String, value: i64, timestamp_ms: u64
//   Then implement `newer_of` using field access.
//   Do not edit the tests.

struct Datapoint {
    path: String,
    value: i64,
    timestamp_ms: u64,
}

fn make_speed(value: i64, timestamp_ms: u64) -> Datapoint {
    Datapoint {
        path: String::from("Vehicle.Speed"),
        value,
        timestamp_ms,
    }
}

/// Returns whichever datapoint has the larger timestamp (the first on a tie).
fn newer_of(a: Datapoint, b: Datapoint) -> Datapoint {
    if b.timestamp_ms > a.timestamp_ms {
        b
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_read() {
        let dp = make_speed(120, 1_000);
        assert_eq!(dp.path, "Vehicle.Speed");
        assert_eq!(dp.value, 120);
        assert_eq!(dp.timestamp_ms, 1_000);
    }

    #[test]
    fn mutate_a_field() {
        let mut dp = make_speed(0, 5);
        dp.value = 42;
        assert_eq!(dp.value, 42);
    }

    #[test]
    fn newest_wins() {
        let newest = newer_of(make_speed(1, 10), make_speed(2, 20));
        assert_eq!(newest.value, 2);
        let tie = newer_of(make_speed(1, 10), make_speed(2, 10));
        assert_eq!(tie.value, 1);
    }
}
