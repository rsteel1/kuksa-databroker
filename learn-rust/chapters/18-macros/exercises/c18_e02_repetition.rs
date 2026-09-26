// c18_e02_repetition.rs
//
// CONCEPT: repetitions `$( ... ),*`, optional trailing commas `$(,)?`,
//          nested repetitions and the `stringify!`/`concat!` built-ins.
// GUIDE:   chapters/18-macros/guide.md  (section "Repetition")
// DATABROKER: databroker/src/main.rs — `concat!(...)` builds the `--version`
//             text at compile time (line 245); `vec![...]` (a std macro built
//             exactly like `hashmap!` below) is used all over broker.rs.
// EXPECT:  compile error "no rules expected `,`" (the macros only
//          accept a single item so far).
//
// `$( $key:expr => $value:expr ),*` means "zero or more `key => value` pairs
// separated by commas". In the template, `$( ... )*` repeats once per match,
// and every metavariable used inside it must come from the same repetition.
//
// YOUR TASK
//   Extend the three macros so that they accept any number of items:
//     - `hashmap!{ k => v, k => v, ... }` (a trailing comma must be allowed),
//       pre-sizing the map with the number of pairs;
//     - `count!(a b c)` counting its tokens at compile time;
//     - `signal_paths!{ Branch => [Leaf, Leaf], Branch => [Leaf] }` producing a
//       `Vec<&'static str>` of dotted paths, e.g. "Vehicle.Speed".
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;

/// Counts the token trees passed to it, as a `usize` constant expression.
macro_rules! count {
    () => {
        0usize
    };
    // TODO: accept any number of token trees, not just one.
    ($x:tt) => {
        1usize
    };
}

/// A `HashMap` literal: `hashmap!{ "a" => 1, "b" => 2 }`.
macro_rules! hashmap {
    // TODO: accept zero or more pairs (and a trailing comma); pre-size the map
    // with `count!`.
    ($key:expr => $value:expr) => {{
        let mut map = HashMap::new();
        map.insert($key, $value);
        map
    }};
}

/// `signal_paths!{ Vehicle => [Speed, IsMoving], Cabin => [Temp] }`
/// expands to `vec!["Vehicle.Speed", "Vehicle.IsMoving", "Cabin.Temp"]`.
macro_rules! signal_paths {
    // TODO: accept many branches, each with zero or more leaves.
    ($branch:ident => [$leaf:ident]) => {
        vec![concat!(stringify!($branch), ".", stringify!($leaf))]
    };
}

fn default_ports() -> HashMap<&'static str, u16> {
    hashmap! {
        "grpc" => 55555,
        "viss" => 8090,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashmap_literal() {
        let m = default_ports();
        assert_eq!(m.len(), 2);
        assert_eq!(m["grpc"], 55555);
        assert_eq!(m["viss"], 8090);
        let empty: HashMap<String, i32> = hashmap! {};
        assert!(empty.is_empty());
    }

    #[test]
    fn hashmap_is_presized() {
        let m = hashmap! { 1 => 'a', 2 => 'b', 3 => 'c', 4 => 'd' };
        assert!(m.capacity() >= 4);
        assert_eq!(m[&3], 'c');
    }

    #[test]
    fn count_is_a_compile_time_constant() {
        const N: usize = count!(Speed Rpm IsMoving);
        assert_eq!(N, 3);
        assert_eq!(count!(), 0);
    }

    #[test]
    fn nested_repetition_builds_paths() {
        let paths = signal_paths! {
            Vehicle => [Speed, IsMoving],
            Chassis => [],
            Cabin => [Temperature],
        };
        assert_eq!(
            paths,
            vec!["Vehicle.Speed", "Vehicle.IsMoving", "Cabin.Temperature"]
        );
    }
}
