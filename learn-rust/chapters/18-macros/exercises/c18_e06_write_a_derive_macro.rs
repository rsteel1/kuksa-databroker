// c18_e06_write_a_derive_macro.rs
//
// CONCEPT: procedural macros: a derive macro written with `syn` (parse
//          tokens into a syntax tree) and `quote!` (build tokens back),
//          with helper attributes and proper compile errors.
// GUIDE:   chapters/18-macros/guide.md  (section "Procedural macros")
// DATABROKER: databroker/src/broker.rs — `#[async_trait::async_trait]` on
//             `SignalProvider` (line 197) is an *attribute* proc macro that
//             rewrites every `async fn` in the trait; `#[tokio::test]` and
//             serde's `#[derive(Deserialize)]` (vss.rs line 39) are too.
// EXPECT:  compile error "c18_e06: #[derive(SignalNames)] on `Cabin` is not
//          implemented yet — edit learn-rust/chapters/18-macros/exercise-macros/src/lib.rs"
//
// THE FILE YOU EDIT IS NOT THIS ONE. The derive lives in the proc-macro crate
// `learn-rust/chapters/18-macros/exercise-macros/src/lib.rs` (proc macros must live in their own
// crate, compiled for the host). This file only *uses* it; its tests pin the
// behaviour. The doc comment at the top of that file shows the exact
// expansion to produce. Reference solution: learn-rust/chapters/18-macros/solution-macros/src/lib.rs.
//
// YOUR TASK
//   Implement `#[derive(SignalNames)]` in chapters/18-macros/exercise-macros/src/lib.rs so that
//   it generates, for a struct with named fields:
//     - `pub const SIGNAL_PATHS: &'static [&'static str]`, one VSS path per
//       field: `#[signal(prefix = "A.B")]` on the struct is joined with the
//       field name converted to UpperCamelCase (`dome_light` → `DomeLight`);
//       `#[signal(rename = "X")]` overrides the leaf name; `#[signal(skip)]`
//       leaves the field out;
//     - `pub fn signal_values(&self) -> Vec<(&'static str, String)>`, pairing
//       each path with the field rendered through `Display`.
//   Generic structs must work. Using the derive on an enum or a tuple struct
//   should be a compile error (you can't test that here; try it by hand).
//   When the tests pass, delete the marker in THIS file. Do not edit the tests.

// I AM NOT DONE

use macros::SignalNames;
use std::fmt::Display;

#[derive(SignalNames)]
#[signal(prefix = "Vehicle.Cabin")]
struct Cabin {
    temperature: f32,
    #[signal(rename = "IsDomeOn")]
    dome_light: bool,
    #[signal(skip)]
    cache: u8,
}

#[derive(SignalNames)]
struct Speedometer {
    vehicle_speed: u32,
    trip_meter_reading: f64,
}

#[derive(SignalNames)]
#[signal(prefix = "Vehicle.Powertrain")]
struct Wrapper<T: Display> {
    state_of_charge: T,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_rename_and_skip() {
        assert_eq!(
            Cabin::SIGNAL_PATHS,
            &["Vehicle.Cabin.Temperature", "Vehicle.Cabin.IsDomeOn"]
        );
    }

    #[test]
    fn no_prefix_uses_upper_camel_case_leaf_names() {
        assert_eq!(
            Speedometer::SIGNAL_PATHS,
            &["VehicleSpeed", "TripMeterReading"]
        );
    }

    #[test]
    fn values_follow_field_display() {
        let cabin = Cabin {
            temperature: 21.5,
            dome_light: true,
            cache: 9,
        };
        assert_eq!(
            cabin.signal_values(),
            vec![
                ("Vehicle.Cabin.Temperature", "21.5".to_string()),
                ("Vehicle.Cabin.IsDomeOn", "true".to_string()),
            ]
        );
        assert_eq!(cabin.cache, 9);
    }

    #[test]
    fn generic_structs_work() {
        let w = Wrapper {
            state_of_charge: 87u8,
        };
        assert_eq!(
            Wrapper::<u8>::SIGNAL_PATHS,
            &["Vehicle.Powertrain.StateOfCharge"]
        );
        assert_eq!(
            w.signal_values(),
            vec![("Vehicle.Powertrain.StateOfCharge", "87".to_string())]
        );
    }
}
