// c18_e01_macro_rules_basics.rs
//
// CONCEPT: `macro_rules!` basics: arms, fragment specifiers (`expr`, `ident`,
//          `ty`), textual order, and "arguments are pasted, not evaluated".
// GUIDE:   chapters/18-macros/guide.md  (section "Declarative macros: macro_rules!")
// DATABROKER: databroker/src/permissions.rs — `lazy_static! { ... }` (line 21)
//             and databroker-proto/src/lib.rs — `tonic::include_proto!(...)`
//             are macro calls; this exercise shows what such a call really is.
// EXPECT:  compile error "cannot find macro `newtype` in this scope". Once
//          that's fixed: "expected identifier, found metavariable". Once that's
//          fixed: two tests fail because an argument was evaluated twice.
//
// A `macro_rules!` macro is a set of pattern => template rules. The compiler
// matches the *tokens* of the call against each pattern, top to bottom, and
// pastes the captured fragments into the template. Three consequences:
//   1. A macro must be defined *above* its first use in the file (unlike
//      functions, which can be anywhere).
//   2. Each capture has a kind (`$x:expr`, `$name:ident`, `$t:ty`, ...) and
//      the template may only use it where that kind is allowed.
//   3. An `expr` is pasted wherever it appears: if the template mentions `$x`
//      twice, the expression runs twice.
//
// YOUR TASK
//   Fix the three macros below so the tests pass:
//     - make `max_of!` and `newtype!` usable (where must they be defined?);
//     - fix the fragment specifier `newtype!` uses for the type name;
//     - make `square!` and `max_of!` evaluate each argument exactly once.
//   `double_counter` shows hygiene; read it, you don't need to change it.
//   Do not edit the tests.

/// `max_of!(a, b)` returns the larger of two values.
macro_rules! max_of {
    ($a:expr, $b:expr) => {{
        // WHY: bind first, so each argument is evaluated exactly once.
        let a = $a;
        let b = $b;
        if a > b {
            a
        } else {
            b
        }
    }};
}

/// `newtype!(Name, Inner)` declares `struct Name(pub Inner);` with a getter.
macro_rules! newtype {
    // WHY: a struct name is an identifier, so capture it as `ident`.
    ($name:ident, $inner:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct $name(pub $inner);

        impl $name {
            fn value(self) -> $inner {
                self.0
            }
        }
    };
}

/// `square!(x)` squares a number, evaluating `x` only once.
macro_rules! square {
    ($x:expr) => {{
        let value = $x;
        value * value
    }};
}

/// Hygiene: the `tmp` inside the macro is a *different* variable from any
/// `tmp` at the call site, even though they have the same name.
macro_rules! double_counter {
    ($counter:ident) => {
        let tmp = $counter * 2;
        $counter = tmp;
    };
}

newtype!(SignalId, i32);
newtype!(Celsius, f64);

fn largest_reading(a: i64, b: i64) -> i64 {
    max_of!(a, b)
}

fn double_counter(start: u32) -> (u32, u32) {
    let tmp = 100; // not touched by the macro's own `tmp`
    let mut counter = start;
    double_counter!(counter);
    (counter, tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_of_works_for_any_comparable_type() {
        assert_eq!(largest_reading(3, 7), 7);
        assert_eq!(max_of!(2.5, -1.0), 2.5);
        assert_eq!(max_of!("Vehicle.Speed", "Vehicle.Cabin"), "Vehicle.Speed");
    }

    #[test]
    fn newtype_generates_a_struct_and_getter() {
        let id = SignalId(42);
        assert_eq!(id.value(), 42);
        assert_eq!(Celsius(21.5).value(), 21.5);
        assert_eq!(format!("{id:?}"), "SignalId(42)");
    }

    #[test]
    fn square_evaluates_its_argument_once() {
        let mut calls = 0;
        let mut next = || {
            calls += 1;
            calls * 3
        };
        assert_eq!(square!(next()), 9);
        assert_eq!(calls, 1, "the argument expression ran more than once");
    }

    #[test]
    fn expr_fragments_keep_their_grouping() {
        // Unlike C preprocessor macros, `1 + 2` is captured as ONE expression,
        // so this is (1 + 2) * (1 + 2), not 1 + 2 * 1 + 2.
        assert_eq!(square!(1 + 2), 9);
    }

    #[test]
    fn hygiene_keeps_macro_locals_separate() {
        assert_eq!(double_counter(21), (42, 100));
    }

    #[test]
    fn max_of_evaluates_each_argument_once() {
        let mut calls = 0;
        let mut bump = || {
            calls += 1;
            calls
        };
        let m = max_of!(bump(), 0);
        assert_eq!(m, 1);
        assert_eq!(calls, 1);
    }
}
