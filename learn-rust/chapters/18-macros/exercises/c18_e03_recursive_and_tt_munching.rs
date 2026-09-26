// c18_e03_recursive_and_tt_munching.rs
//
// CONCEPT: recursive macros, internal `@rules`, "tt munching" (consume the
//          input one token tree at a time, carrying an accumulator) and
//          `compile_error!` for bad input.
// GUIDE:   chapters/18-macros/guide.md  (section "Recursion and tt munching")
// DATABROKER: doc/wildcard_matching.md and databroker/src/glob.rs — the VSS
//             wildcard syntax (`Vehicle.*.IsOpen`, `Vehicle.**`) that
//             `glob!` below parses at compile time.
// EXPECT:  compile errors "no rules expected `9`" (max_of!) and
//          "recursion limit reached while expanding `glob!`" — an internal
//          `@acc` call that matches no internal rule falls through to the
//          public catch-all rule, which wraps it again, forever.
//
// Real macros like `serde_json::json!` and `tokio::select!` are written this
// way: a public entry rule forwards to internal rules (by convention they
// start with `@`), which chew through the input and call themselves again
// with the rest. Each recursive call is one step of a loop that runs inside
// the compiler.
//
// YOUR TASK
//   Complete the three macros so the tests pass:
//     - `max_of!(a, b, c, ...)` for any number of arguments (recursively);
//     - `glob!(Vehicle.*.IsOpen)` turning a wildcard path written as tokens
//       into a `&'static str` (handle identifiers, `.`, `*` and `**`);
//     - `signals!{ sensor A.B: f32 = "..."; actuator ...; attribute ...; }`
//       building a `Vec<SignalDecl>`.
//   Do not edit `SignalDecl`, `Kind` or the tests.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Sensor,
    Actuator,
    Attribute,
}

#[derive(Debug, Clone, PartialEq)]
struct SignalDecl {
    path: &'static str,
    kind: Kind,
    type_name: &'static str,
    description: &'static str,
}

/// The largest of one or more expressions, each evaluated exactly once.
macro_rules! max_of {
    // TODO: add a second rule for two or more arguments that recurses on the rest.
    ($x:expr $(,)?) => {
        $x
    };
}

/// Turns wildcard-path tokens into a string at compile time.
macro_rules! glob {
    // Internal rules. The accumulator `[...]` holds string pieces so far.
    (@acc [$($out:expr),*]) => {
        concat!($($out),*)
    };
    // TODO: rules for `**` and `*` (careful: which must come first?).
    (@acc [$($out:expr),*] .. $($rest:tt)*) => {
        compile_error!("empty path segment: `..` is not allowed in a VSS path")
    };
    (@acc [$($out:expr),*] . $($rest:tt)*) => {
        glob!(@acc [$($out,)* "."] $($rest)*)
    };
    (@acc [$($out:expr),*] $segment:ident $($rest:tt)*) => {
        glob!(@acc [$($out,)* stringify!($segment)] $($rest)*)
    };
    // Public entry point: start with an empty accumulator.
    ($($tokens:tt)+) => {
        glob!(@acc [] $($tokens)+)
    };
}

/// A declaration DSL. Each statement is `<kind> <path>: <type> = "<description>";`.
macro_rules! signals {
    (@acc [$($done:expr,)*]) => {
        vec![$($done,)*]
    };
    // TODO: `sensor` and `attribute` statements.
    (@acc [$($done:expr,)*] actuator $($rest:tt)*) => {
        signals!(@decl [$($done,)*] Kind::Actuator, $($rest)*)
    };
    // TODO: an `@decl` rule that parses `path: type = "description";`,
    // pushes a `SignalDecl` onto the accumulator and continues with `@acc`.
    ($($tokens:tt)*) => {
        signals!(@acc [] $($tokens)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_of_many() {
        assert_eq!(max_of!(4), 4);
        assert_eq!(max_of!(3, 9, 2), 9);
        assert_eq!(max_of!(1.5, -2.0, 7.25, 7.0,), 7.25);
    }

    #[test]
    fn max_of_evaluates_each_argument_once() {
        let mut calls = 0;
        let mut next = || {
            calls += 1;
            calls
        };
        assert_eq!(max_of!(next(), next(), next()), 3);
        assert_eq!(calls, 3);
    }

    #[test]
    fn glob_plain_path() {
        assert_eq!(glob!(Vehicle.Speed), "Vehicle.Speed");
    }

    #[test]
    fn glob_wildcards() {
        assert_eq!(glob!(Vehicle.*.IsOpen), "Vehicle.*.IsOpen");
        assert_eq!(glob!(Vehicle.**), "Vehicle.**");
        assert_eq!(glob!(**.IsOpen), "**.IsOpen");
        assert_eq!(glob!(Vehicle.**.Row1.*), "Vehicle.**.Row1.*");
    }

    #[test]
    fn glob_is_a_compile_time_constant() {
        const PATTERN: &str = glob!(Vehicle.Cabin.*);
        assert_eq!(PATTERN, "Vehicle.Cabin.*");
    }

    #[test]
    fn signal_dsl() {
        let decls = signals! {
            sensor Vehicle.Speed: f32 = "Vehicle speed";
            actuator Vehicle.Cabin.Door.Row1.IsOpen: bool = "Is the door open";
            attribute Vehicle.VehicleIdentification.VIN: String = "VIN";
        };
        assert_eq!(decls.len(), 3);
        assert_eq!(
            decls[0],
            SignalDecl {
                path: "Vehicle.Speed",
                kind: Kind::Sensor,
                type_name: "f32",
                description: "Vehicle speed",
            }
        );
        assert_eq!(decls[1].path, "Vehicle.Cabin.Door.Row1.IsOpen");
        assert_eq!(decls[1].kind, Kind::Actuator);
        assert_eq!(decls[2].kind, Kind::Attribute);
        assert_eq!(decls[2].type_name, "String");
    }

    #[test]
    fn empty_signal_dsl() {
        let decls: Vec<SignalDecl> = signals! {};
        assert!(decls.is_empty());
    }
}
