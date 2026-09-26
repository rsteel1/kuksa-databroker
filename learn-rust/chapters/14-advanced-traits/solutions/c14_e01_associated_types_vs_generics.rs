// c14_e01_associated_types_vs_generics.rs
//
// CONCEPT: associated types (`type Item;` — ONE per implementation) vs
//          generic trait parameters (`From<T>` — MANY per type).
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Associated types vs generic parameters")
// DATABROKER: databroker/src/broker.rs — `impl Iterator for EntryReadIterator`
//             sets `type Item = EntryReadAccess<'a>`; databroker-proto/src/lib.rs
//             `impl FromStr for DataType` sets `type Err = ParsingError`;
//             databroker/src/grpc/kuksa_val_v2/conversions.rs implements `From`
//             many times for the same types (one impl per source type).
// EXPECT:  compile errors E0277 (`DoorSensor: SignalSource` and
//          `Reading: ConvertFrom<bool>` are not satisfied), then `todo!()` panics.
//
// Ask: "for one implementing type, can there be several answers?"
//   A sensor produces ONE kind of value  -> associated type (`type Value`).
//   A Reading can be built FROM several kinds of input -> generic `ConvertFrom<T>`.
// With an associated type, callers write `S::Value` and the compiler knows
// exactly which type that is — no annotations needed.
//
// YOUR TASK
//   1. Implement `SignalSource` for `DoorSensor` (its values are `bool`).
//   2. Implement `drain` (collect every remaining value).
//   3. Add the missing `impl ConvertFrom<bool> for Reading`
//      (true -> Reading::Flag(true), ...).
//   Do not edit the tests.

/// Something that yields values of ONE type.
trait SignalSource {
    type Value;
    fn next_value(&mut self) -> Option<Self::Value>;
}

/// Something that can be built from a `T` — possibly for many different `T`s.
trait ConvertFrom<T> {
    fn convert(from: T) -> Self;
}

struct SpeedSensor {
    samples: Vec<f32>,
}

impl SignalSource for SpeedSensor {
    type Value = f32;
    fn next_value(&mut self) -> Option<f32> {
        if self.samples.is_empty() {
            None
        } else {
            Some(self.samples.remove(0))
        }
    }
}

struct DoorSensor {
    toggles_left: u32,
    open: bool,
}

impl SignalSource for DoorSensor {
    type Value = bool;
    fn next_value(&mut self) -> Option<bool> {
        if self.toggles_left == 0 {
            return None;
        }
        self.toggles_left -= 1;
        self.open = !self.open;
        Some(self.open)
    }
}

/// Pull values until the source is exhausted.
/// Note the return type: `S::Value` — no extra generic parameter needed.
fn drain<S: SignalSource>(source: &mut S) -> Vec<S::Value> {
    let mut out = Vec::new();
    while let Some(value) = source.next_value() {
        out.push(value);
    }
    out
}

#[derive(Debug, PartialEq)]
enum Reading {
    Number(f64),
    Flag(bool),
}

impl ConvertFrom<f32> for Reading {
    fn convert(from: f32) -> Self {
        Reading::Number(f64::from(from))
    }
}

impl ConvertFrom<bool> for Reading {
    fn convert(from: bool) -> Self {
        Reading::Flag(from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_sensor_drains_in_order() {
        let mut s = SpeedSensor {
            samples: vec![10.0, 20.5],
        };
        assert_eq!(drain(&mut s), vec![10.0, 20.5]);
        assert_eq!(s.next_value(), None);
    }

    #[test]
    fn door_sensor_toggles() {
        let mut d = DoorSensor {
            toggles_left: 3,
            open: false,
        };
        let values: Vec<bool> = drain(&mut d);
        assert_eq!(values, vec![true, false, true]);
    }

    #[test]
    fn one_type_many_conversions() {
        assert_eq!(Reading::convert(2.5_f32), Reading::Number(2.5));
        assert_eq!(
            <Reading as ConvertFrom<bool>>::convert(true),
            Reading::Flag(true)
        );
        let r: Reading = ConvertFrom::convert(false);
        assert_eq!(r, Reading::Flag(false));
    }
}
