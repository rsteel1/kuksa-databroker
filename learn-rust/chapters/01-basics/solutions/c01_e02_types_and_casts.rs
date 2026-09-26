// c01_e02_types_and_casts.rs
//
// CONCEPT: integer widths, overflow, `as` vs `try_from`.
// GUIDE:   chapters/01-basics/guide.md  (section "Scalar types and conversions")
// DATABROKER: databroker/src/broker.rs — `Entry::validate_value`, the
//             `DataType::Int8 => ... i8::try_from(*value)` arm.
// EXPECT:  tests fail: "attempt to add with overflow" and a wrong Int8 value.
//
// The databroker stores every small integer (Int8, Int16, Uint8, ...) inside
// a 32-bit `DataValue::Int32` / `Uint32`, and checks on every write that the
// value really fits the declared VSS type. It uses `i8::try_from(value)` for
// that — NOT `value as i8`, because `as` silently chops off bits.
//
// YOUR TASK
//   1. `average_speed` overflows: a `u8` can only hold 0..=255. Sum into a
//      wider type (`u32`) using `u32::from(...)`, then convert the result back.
//   2. `to_int8` must return `None` when the value does not fit in an `i8`.
//      Replace the `as` cast with `i8::try_from(value).ok()`.
//   Do not edit the tests.

/// Average of the readings (km/h). Readings are 0..=255 each; the average of
/// such values always fits in a `u8` again.
fn average_speed(readings: &[u8]) -> u8 {
    if readings.is_empty() {
        return 0;
    }
    let mut total: u32 = 0;
    for reading in readings {
        // WHY: u32::from is a *lossless* widening conversion — it can't fail.
        total += u32::from(*reading);
    }
    let average = total / readings.len() as u32;
    // WHY: the average of u8 values is <= 255, so this cannot fail.
    u8::try_from(average).expect("average of u8 values fits in u8")
}

/// Converts an `Int32` payload into a VSS `Int8`, if it fits.
fn to_int8(value: i32) -> Option<i8> {
    i8::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_of_small_values() {
        assert_eq!(average_speed(&[10, 20, 30]), 20);
    }

    #[test]
    fn average_does_not_overflow() {
        assert_eq!(average_speed(&[200, 250, 220]), 223);
    }

    #[test]
    fn average_of_nothing_is_zero() {
        assert_eq!(average_speed(&[]), 0);
    }

    #[test]
    fn int8_in_range() {
        assert_eq!(to_int8(-128), Some(-128));
        assert_eq!(to_int8(127), Some(127));
    }

    #[test]
    fn int8_out_of_range_is_rejected() {
        assert_eq!(to_int8(200), None);
        assert_eq!(to_int8(-129), None);
    }
}
