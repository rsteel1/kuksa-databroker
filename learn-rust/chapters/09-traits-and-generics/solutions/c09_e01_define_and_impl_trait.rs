// c09_e01_define_and_impl_trait.rs
//
// CONCEPT: defining a trait and implementing it for several types.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Traits: shared behaviour")
// DATABROKER: databroker/src/broker.rs — `pub trait ActuationProvider { ...;
//             fn is_available(&self) -> bool; }`, implemented by the gRPC
//             `Provider` in databroker/src/grpc/kuksa_val_v2/val.rs and by
//             several test doubles at the bottom of broker.rs.
// EXPECT:  compile error E0277 (the trait bound `Door: Describe` is not satisfied)
//
// A trait is a named set of methods a type promises to provide. Code that
// only needs "something that can describe itself" can then accept any type
// implementing the trait, without knowing the concrete type.
//
// YOUR TASK
//   Implement `Describe` for `Door` and `SpeedSensor` so `report` works.
//   Formats (checked by tests):
//     Door:        "Door Row1.Left: open" / "Door Row1.Left: closed"
//     SpeedSensor: "Speed: 88.5 km/h"
//   Do not edit `report` or the tests.

trait Describe {
    /// Short, human-readable summary of the current state.
    fn describe(&self) -> String;
}

struct Door {
    position: String,
    is_open: bool,
}

struct SpeedSensor {
    kmh: f32,
}

impl Describe for Door {
    fn describe(&self) -> String {
        let state = if self.is_open { "open" } else { "closed" };
        format!("Door {}: {state}", self.position)
    }
}

impl Describe for SpeedSensor {
    fn describe(&self) -> String {
        format!("Speed: {} km/h", self.kmh)
    }
}

/// Works for ANY type that implements Describe.
fn report(item: &impl Describe) -> String {
    format!("[report] {}", item.describe())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door() {
        let d = Door {
            position: "Row1.Left".into(),
            is_open: true,
        };
        assert_eq!(d.describe(), "Door Row1.Left: open");
        let d = Door {
            position: "Row1.Left".into(),
            is_open: false,
        };
        assert_eq!(report(&d), "[report] Door Row1.Left: closed");
    }

    #[test]
    fn speed() {
        assert_eq!(
            report(&SpeedSensor { kmh: 88.5 }),
            "[report] Speed: 88.5 km/h"
        );
    }
}
