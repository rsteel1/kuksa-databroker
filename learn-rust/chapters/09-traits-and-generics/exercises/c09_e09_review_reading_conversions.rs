// c09_e09_review_reading_conversions.rs
//
// CONCEPT: code review. The file below COMPILES and looks reasonable, but
//          has exactly 3 planted bugs typical of trait/generic code: a lossy
//          numeric conversion, a manual trait impl that doesn't match its
//          own doc comment, and a generic function with a flipped comparison.
// GUIDE:   chapters/09-traits-and-generics/guide.md  (section "Conversions: From, Into, TryFrom")
// DATABROKER: databroker/src/types.rs — the `Int8`/`Uint8`-style range-checked
//          numeric conversions this file's `From<RawSample> for Reading`
//          is supposed to imitate (clamp, don't wrap).
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly — the mistakes are
// logical, not type errors — but they make specific tests fail.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests, the struct/trait
//   signatures, or add/remove anything. For each bug, add a one-line
//   comment directly above your fix: `// BUG 1: <what was wrong>`,
//   `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

use std::cmp::Ordering;

/// A sample as it arrives from a provider: id plus millivolts, both as
/// whatever width the wire format used (wider than what we store).
struct RawSample {
    id: u32,
    millivolts: i32,
}

/// A stored reading. `millivolts` is deliberately narrow (`i16`) to save
/// space; out-of-range samples must be clamped to the nearest representable
/// value, never silently wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reading {
    id: u32,
    millivolts: i16,
}

impl From<RawSample> for Reading {
    fn from(raw: RawSample) -> Self {
        Reading {
            id: raw.id,
            millivolts: raw.millivolts as i16,
        }
    }
}

/// Readings order by voltage first, ties broken by id — so the lowest
/// voltage always sorts first regardless of which id reported it.
impl PartialOrd for Reading {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Reading {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

/// Clamps every element of `items` into `[lo, hi]`, in place.
fn clamp_all<T: PartialOrd + Copy>(items: &mut [T], lo: T, hi: T) {
    for item in items.iter_mut() {
        if *item < lo {
            *item = lo;
        }
        if *item < hi {
            *item = hi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_clamps_instead_of_wrapping() {
        // i16::MAX is 32767; a wire value above that must clamp, not wrap.
        let raw = RawSample {
            id: 1,
            millivolts: 40_000,
        };
        let reading: Reading = raw.into();
        assert_eq!(reading.millivolts, i16::MAX);

        let raw_low = RawSample {
            id: 2,
            millivolts: -40_000,
        };
        let reading_low: Reading = raw_low.into();
        assert_eq!(reading_low.millivolts, i16::MIN);
    }

    #[test]
    fn ordering_is_by_voltage_not_by_id() {
        let mut readings = [
            Reading {
                id: 1,
                millivolts: 500,
            },
            Reading {
                id: 2,
                millivolts: 100,
            },
            Reading {
                id: 3,
                millivolts: 300,
            },
        ];
        readings.sort();
        let ids: Vec<u32> = readings.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 3, 1]); // by ascending millivolts: 100, 300, 500
    }

    #[test]
    fn ordering_breaks_ties_by_id() {
        let mut readings = [
            Reading {
                id: 5,
                millivolts: 200,
            },
            Reading {
                id: 4,
                millivolts: 200,
            },
        ];
        readings.sort();
        let ids: Vec<u32> = readings.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![4, 5]);
    }

    #[test]
    fn clamp_all_pins_values_into_range() {
        let mut values = [-5, 0, 5, 10, 15];
        clamp_all(&mut values, 0, 10);
        assert_eq!(values, [0, 0, 5, 10, 10]);
    }
}
