// c04_e06_match_on_tuples.rs
//
// CONCEPT: matching on a tuple of two enums at once; lossless widening
//          (`i64::from`, `f64::from`); `Option::map`.
// GUIDE:   chapters/04-enums/guide.md  (section "Matching two values at once")
// DATABROKER: databroker/src/types.rs — `DataValue::greater_than` does
//             `match (&self, other) { (Int32(v), Int64(o)) => Ok(i64::from(*v) > *o), ... }`
//             and is used by `validate_value_min_max` in broker.rs.
// EXPECT:  tests fail: `todo!()` panics.
//
// A signal's `min`/`max` metadata may have a different numeric variant than
// the value being written (e.g. value `Uint32(5)`, min `Int32(-10)`). To
// compare them, convert both to a type that can hold both ranges without
// loss: i64 for i32/u32 pairs, f64 when a Double is involved.
//
// The databroker returns `Result<bool, CastError>`; we use `Option<bool>`
// (None = "these two can't be compared") because `Result` is chapter 07.
//
// YOUR TASK
//   Implement `greater_than`, then `less_than_or_equal` in terms of it.
//   Rules:
//     - integers (Int32, Uint32, Int64 in any combination): compare as i64
//       (use the provided `as_i64` helper).
//     - any integer vs Double, or Double vs Double: compare as f64.
//     - String vs String: lexicographic (`>` on &str works).
//     - Bool vs Bool: `true > false`.
//     - everything else (including anything with NotAvailable): None.
//   Do not edit the tests.

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    NotAvailable,
    Bool(bool),
    String(String),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Double(f64),
}

impl DataValue {
    /// Integers widened to i64 (all three integer variants fit).
    fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Int32(v) => Some(i64::from(*v)),
            DataValue::Uint32(v) => Some(i64::from(*v)),
            DataValue::Int64(v) => Some(*v),
            _ => None,
        }
    }

    fn greater_than(&self, other: &DataValue) -> Option<bool> {
        match (self, other) {
            (DataValue::Bool(a), DataValue::Bool(b)) => Some(a > b),
            (DataValue::String(a), DataValue::String(b)) => Some(a > b),
            (DataValue::Double(a), DataValue::Double(b)) => Some(a > b),
            // WHY: one side is a Double: widen the other side if it is an
            // integer (i64 -> f64 may round huge values; the databroker
            // accepts the same trade-off). `map` keeps None as None.
            (DataValue::Double(a), b) => b.as_i64().map(|b| *a > b as f64),
            (a, DataValue::Double(b)) => a.as_i64().map(|a| a as f64 > *b),
            // Everything else: comparable only if both are integers.
            (a, b) => match (a.as_i64(), b.as_i64()) {
                (Some(a), Some(b)) => Some(a > b),
                _ => None,
            },
        }
    }

    /// `self <= other`, i.e. "not greater than" — but only when comparable.
    fn less_than_or_equal(&self, other: &DataValue) -> Option<bool> {
        self.greater_than(other).map(|greater| !greater)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DataValue::*;

    #[test]
    fn same_variant() {
        assert_eq!(Int32(5).greater_than(&Int32(3)), Some(true));
        assert_eq!(Int32(3).greater_than(&Int32(3)), Some(false));
        assert_eq!(Double(1.5).greater_than(&Double(0.5)), Some(true));
        assert_eq!(Bool(true).greater_than(&Bool(false)), Some(true));
        assert_eq!(
            String("b".into()).greater_than(&String("a".into())),
            Some(true)
        );
    }

    #[test]
    fn mixed_integers() {
        assert_eq!(Uint32(5).greater_than(&Int32(-10)), Some(true));
        assert_eq!(Int32(-1).greater_than(&Uint32(0)), Some(false));
        assert_eq!(Uint32(u32::MAX).greater_than(&Int32(i32::MAX)), Some(true));
        assert_eq!(
            Int64(10_000_000_000).greater_than(&Uint32(u32::MAX)),
            Some(true)
        );
    }

    #[test]
    fn integers_and_doubles() {
        assert_eq!(Int32(3).greater_than(&Double(2.5)), Some(true));
        assert_eq!(Double(2.5).greater_than(&Uint32(3)), Some(false));
    }

    #[test]
    fn incomparable() {
        assert_eq!(Int32(1).greater_than(&Bool(true)), None);
        assert_eq!(String("1".into()).greater_than(&Int32(0)), None);
        assert_eq!(NotAvailable.greater_than(&Int32(0)), None);
        assert_eq!(Int32(0).greater_than(&NotAvailable), None);
    }

    #[test]
    fn min_max_check() {
        // value 50 within min 0 / max 100?
        let value = Uint32(50);
        assert_eq!(value.less_than_or_equal(&Int32(100)), Some(true));
        assert_eq!(value.less_than_or_equal(&Int32(10)), Some(false));
        assert_eq!(value.less_than_or_equal(&Bool(false)), None);
    }
}
