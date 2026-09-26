// c01_e04_control_flow.rs
//
// CONCEPT: `if` as an expression, `for`, `while`, `loop` with a break value.
// GUIDE:   chapters/01-basics/guide.md  (section "Control flow")
// DATABROKER: databroker/src/broker.rs — `start_housekeeping_task` is one big
//             `loop { interval.tick().await; ... }` that runs forever.
// EXPECT:  tests fail: `todo!()` panics with "not yet implemented".
//
// `todo!()` is a macro that compiles anywhere (it "returns" any type) and
// panics when it runs. It's a handy placeholder while you write code.
//
// YOUR TASK
//   Replace each `todo!()` with a real implementation. Do not edit the tests.

/// Suggested gear for a speed in km/h:
///   0..=15 -> 1, 16..=30 -> 2, 31..=50 -> 3, 51..=70 -> 4, above -> 5
/// Use `if` / `else if` / `else` as an expression.
fn gear_for_speed(kmh: u32) -> u8 {
    if kmh <= 15 {
        1
    } else if kmh <= 30 {
        2
    } else if kmh <= 50 {
        3
    } else if kmh <= 70 {
        4
    } else {
        5
    }
}

/// Sums values until the sentinel `-1` is seen (the sentinel is not added).
/// Use a `for` loop and `break`.
fn sum_until_sentinel(values: &[i32]) -> i32 {
    let mut sum = 0;
    for &value in values {
        if value == -1 {
            break;
        }
        sum += value;
    }
    sum
}

/// Smallest power of two that is >= `n` (for n >= 1). E.g. 5 -> 8, 8 -> 8.
/// Use `loop` and return the result with `break value`.
fn next_power_of_two(n: u32) -> u32 {
    let mut candidate = 1;
    loop {
        if candidate >= n {
            break candidate;
        }
        candidate *= 2;
    }
}

/// How many times can `budget` be halved (integer division) before it
/// reaches 0? E.g. 8 -> 8,4,2,1 -> 4 halvings. Use `while`.
fn halvings(mut budget: u32) -> u32 {
    let mut count = 0;
    while budget > 0 {
        budget /= 2;
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gears() {
        assert_eq!(gear_for_speed(0), 1);
        assert_eq!(gear_for_speed(15), 1);
        assert_eq!(gear_for_speed(16), 2);
        assert_eq!(gear_for_speed(50), 3);
        assert_eq!(gear_for_speed(70), 4);
        assert_eq!(gear_for_speed(71), 5);
        assert_eq!(gear_for_speed(250), 5);
    }

    #[test]
    fn sentinel() {
        assert_eq!(sum_until_sentinel(&[1, 2, 3, -1, 100]), 6);
        assert_eq!(sum_until_sentinel(&[5, 5]), 10);
        assert_eq!(sum_until_sentinel(&[-1, 7]), 0);
    }

    #[test]
    fn powers() {
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(8), 8);
        assert_eq!(next_power_of_two(1000), 1024);
    }

    #[test]
    fn halving() {
        assert_eq!(halvings(0), 0);
        assert_eq!(halvings(1), 1);
        assert_eq!(halvings(8), 4);
        assert_eq!(halvings(1000), 10);
    }
}
