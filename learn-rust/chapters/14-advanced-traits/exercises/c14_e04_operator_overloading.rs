// c14_e04_operator_overloading.rs
//
// CONCEPT: operators are traits: `a + b` is `Add::add(a, b)`, `-a` is
//          `Neg`, `t["x"]` is `Index`, `<` comes from `PartialOrd`/`Ord`.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Operator overloading")
// DATABROKER: databroker/src/types.rs — `DataValue::greater_than`,
//             `less_than`, `equals` are *methods* returning
//             `Result<bool, CastError>` rather than `PartialOrd`, because
//             comparing a String with an Int must be an error, not `false`.
//             (`PartialOrd::partial_cmp` returning `None` could have
//             expressed "not comparable" — a design trade-off worth discussing.)
// EXPECT:  tests fail: `todo!()` panics
//
// YOUR TASK
//   1. `Speed` (km/h): implement `Add`, `Sub`, `Neg`, `Mul<f64>` and `AddAssign`.
//   2. `SignalTable`: implement `Index<&str>` (panics with a message naming the
//      missing path, like `HashMap` does) and `IndexMut<&str>`.
//   3. `Priority`: a LOWER number means MORE important. Implement `Ord` so that
//      sorting puts the most important first, and derive-free `PartialOrd`
//      that agrees with it. (`PartialEq`/`Eq` are derived.)
//   Do not edit the tests.

// I AM NOT DONE

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Add, AddAssign, Index, IndexMut, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Speed(f64);

impl Add for Speed {
    type Output = Speed;
    fn add(self, rhs: Speed) -> Speed {
        todo!()
    }
}

impl Sub for Speed {
    type Output = Speed;
    fn sub(self, rhs: Speed) -> Speed {
        todo!()
    }
}

impl Neg for Speed {
    type Output = Speed;
    fn neg(self) -> Speed {
        todo!()
    }
}

/// Scaling by a plain number: `Speed(10.0) * 1.5`.
impl Mul<f64> for Speed {
    type Output = Speed;
    fn mul(self, factor: f64) -> Speed {
        todo!()
    }
}

impl AddAssign for Speed {
    fn add_assign(&mut self, rhs: Speed) {
        todo!()
    }
}

#[derive(Default)]
struct SignalTable {
    values: HashMap<String, f64>,
}

impl Index<&str> for SignalTable {
    type Output = f64;
    fn index(&self, path: &str) -> &f64 {
        todo!()
    }
}

impl IndexMut<&str> for SignalTable {
    fn index_mut(&mut self, path: &str) -> &mut f64 {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Priority(u8);

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        todo!()
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_arithmetic() {
        assert_eq!(Speed(10.0) + Speed(5.0), Speed(15.0));
        assert_eq!(Speed(10.0) - Speed(15.0), Speed(-5.0));
        assert_eq!(-Speed(3.0), Speed(-3.0));
        assert_eq!(Speed(10.0) * 1.5, Speed(15.0));
        let mut s = Speed(1.0);
        s += Speed(2.0);
        assert_eq!(s, Speed(3.0));
    }

    #[test]
    fn table_indexing() {
        let mut table = SignalTable::default();
        table["Vehicle.Speed"] = 42.0;
        table["Vehicle.Speed"] += 1.0;
        assert_eq!(table["Vehicle.Speed"], 43.0);
    }

    #[test]
    #[should_panic(expected = "no signal named Vehicle.Rpm")]
    fn missing_signal_panics_with_its_name() {
        let table = SignalTable::default();
        let _ = table["Vehicle.Rpm"];
    }

    #[test]
    fn most_important_first() {
        let mut ps = vec![Priority(3), Priority(1), Priority(2)];
        ps.sort_by(|a, b| b.cmp(a));
        assert_eq!(ps, vec![Priority(1), Priority(2), Priority(3)]);
        assert!(Priority(1) > Priority(9));
        assert_eq!(ps.iter().max(), Some(&Priority(1)));
    }
}
