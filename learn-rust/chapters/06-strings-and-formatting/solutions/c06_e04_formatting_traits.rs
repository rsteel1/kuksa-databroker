// c06_e04_formatting_traits.rs
//
// CONCEPT: `Display` (for people) vs `Debug` (for programmers), format specs
//          like `{:>8.2}` / `{:#?}` / `{:#06x}`, and honouring width in your
//          own `Display` with `Formatter::pad`.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Formatting: Display, Debug and format specs")
// DATABROKER: databroker/src/types.rs — `impl fmt::Display for DataType` and
//             `impl fmt::Display for DataValue` (arrays fall back to `{values:?}`).
// EXPECT:  compile error E0277 (`Reading` doesn't implement `Debug`), then
//          `todo!()` panics.
//
// `println!("{}", x)` needs `x: Display`; `println!("{:?}", x)` needs
// `x: Debug`. Debug can be derived; Display never can — only you know how
// your type should look to a human.
//
// YOUR TASK
//   1. Make `Reading` printable with `{:?}` / `{:#?}` (one attribute).
//   2. Implement the two `Display` impls (see the doc comments).
//   3. Implement `format_row` and `id_hex` with format specs only.
//   Do not edit the tests.

use std::fmt;

#[derive(Debug)]
struct Reading {
    path: String,
    value: f64,
    unit: Option<String>,
}

/// "Vehicle.Speed = 42.50 km/h", or "Vehicle.Speed = 42.50" without a unit.
/// Always two decimals.
impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // WHY: `write!` into the Formatter returns fmt::Result; each arm
        // returns it directly (chapter 07 shows the `?` shortcut).
        match &self.unit {
            Some(unit) => write!(f, "{} = {:.2} {}", self.path, self.value, unit),
            None => write!(f, "{} = {:.2}", self.path, self.value),
        }
    }
}

/// A unit symbol such as "km/h". Its Display must respect width and
/// alignment: `format!("[{:>6}]", unit)` == "[  km/h]".
struct Unit(String);

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // WHY: `write!(f, "{}", self.0)` would ignore the caller's `{:>6}`;
        // `pad` applies width, fill, alignment and precision for us.
        f.pad(&self.0)
    }
}

/// Path left-aligned in 20 columns, a '|', value right-aligned in 8 columns
/// with 2 decimals: "Vehicle.Speed       |    7.89"
fn format_row(path: &str, value: f64) -> String {
    format!("{path:<20}|{value:>8.2}")
}

/// 255 -> "0x00ff" (lower-case hex, "0x" prefix, total width 6, zero padded)
fn id_hex(id: u32) -> String {
    format!("{id:#06x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speed(unit: Option<&str>) -> Reading {
        Reading {
            path: "Vehicle.Speed".to_string(),
            value: 42.5,
            unit: unit.map(String::from),
        }
    }

    #[test]
    fn display_reading() {
        assert_eq!(
            speed(Some("km/h")).to_string(),
            "Vehicle.Speed = 42.50 km/h"
        );
        assert_eq!(speed(None).to_string(), "Vehicle.Speed = 42.50");
    }

    #[test]
    fn debug_is_for_programmers() {
        let compact = format!("{:?}", speed(None));
        assert!(compact.starts_with("Reading { path: \"Vehicle.Speed\""));
        let pretty = format!("{:#?}", speed(None));
        assert!(pretty.contains('\n'), "{{:#?}} spreads fields over lines");
    }

    #[test]
    fn unit_honours_width_and_alignment() {
        assert_eq!(format!("[{:>6}]", Unit("km/h".into())), "[  km/h]");
        assert_eq!(format!("[{:<6}]", Unit("V".into())), "[V     ]");
        assert_eq!(format!("[{:^5}]", Unit("A".into())), "[  A  ]");
        assert_eq!(format!("[{}]", Unit("°C".into())), "[°C]");
    }

    #[test]
    fn rows_and_hex() {
        assert_eq!(
            format_row("Vehicle.Speed", 7.891),
            "Vehicle.Speed       |    7.89"
        );
        assert_eq!(
            format_row("Vehicle.Rpm", 1800.0),
            "Vehicle.Rpm         | 1800.00"
        );
        assert_eq!(id_hex(255), "0x00ff");
        assert_eq!(id_hex(7), "0x0007");
    }
}
