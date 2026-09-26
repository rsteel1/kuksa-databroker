// c06_e02_string_building.rs
//
// CONCEPT: building strings: `format!`, `push_str`, `join`, `write!` into a
//          `String`, `with_capacity` — and why `+` moves its left operand.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Building strings")
// DATABROKER: databroker/src/glob.rs — `Matcher::to_glob_string` builds its
//             output with `String::new()` + `push_str`; databroker/src/main.rs
//             builds the `--version` text with `format!(concat!(...))`.
// EXPECT:  compile error E0382 (borrow of moved value: `branch`), then
//          `todo!()` panics.
//
// `a + b` on strings is really `a.add(&b)`: it takes `a` BY VALUE (reusing
// its buffer) and borrows `b`. After `let p = branch + ".";` the name
// `branch` is gone. `format!` borrows everything and allocates a new String.
//
// YOUR TASK
//   1. Make `report` compile without `.clone()` (hint: `format!`).
//   2. Implement `join_path` with one call on the slice.
//   3. Implement `render_table`: one line "path=value" per row, each ending
//      in '\n', values with exactly one decimal. Build ONE String (allocate it
//      with `String::with_capacity`) and append with `writeln!`.
//   Do not edit the tests.

// I AM NOT DONE

// `writeln!(some_string, ...)` needs this import. It brings the `Write`
// *trait* into scope — traits are chapter 09; for now read it as
// "switch on the ability to write! into a String".
use std::fmt::Write;

/// "Vehicle.Cabin" + "Door" -> "Vehicle.Cabin.Door (branch Vehicle.Cabin)"
fn report(branch: String, leaf: &str) -> String {
    let path = branch + "." + leaf;
    format!("{} (branch {})", path, branch)
}

/// ["Vehicle", "Cabin", "Door"] -> "Vehicle.Cabin.Door"
fn join_path(parts: &[&str]) -> String {
    todo!()
}

/// [("Vehicle.Speed", 42.0), ("Vehicle.Rpm", 1800.25)] ->
/// "Vehicle.Speed=42.0\nVehicle.Rpm=1800.2\n"
fn render_table(rows: &[(&str, f64)]) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_keeps_branch_usable() {
        assert_eq!(
            report(String::from("Vehicle.Cabin"), "Door"),
            "Vehicle.Cabin.Door (branch Vehicle.Cabin)"
        );
    }

    #[test]
    fn join_uses_dots() {
        assert_eq!(
            join_path(&["Vehicle", "Cabin", "Door"]),
            "Vehicle.Cabin.Door"
        );
        assert_eq!(join_path(&["Vehicle"]), "Vehicle");
        assert_eq!(join_path(&[]), "");
    }

    #[test]
    fn table_has_one_line_per_row() {
        let out = render_table(&[("Vehicle.Speed", 42.0), ("Vehicle.Rpm", 1800.25)]);
        assert_eq!(out, "Vehicle.Speed=42.0\nVehicle.Rpm=1800.2\n");
        assert_eq!(render_table(&[]), "");
    }
}
