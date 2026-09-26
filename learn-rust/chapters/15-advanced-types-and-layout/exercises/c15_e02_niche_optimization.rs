// c15_e02_niche_optimization.rs
//
// CONCEPT: niches: why `Option<&T>` costs nothing, and how to give your own types a niche
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Niches: Option for free")
// DATABROKER: databroker/src/types.rs — `SignalId` wraps a plain `i32`. The first id
//             `Database::add` hands out is 0 (`next_id.fetch_add` in broker.rs returns
//             the old value), so the real broker has no niche to exploit. Here our
//             ids start at 1, so we can.
// EXPECT:  tests fail: `todo!()` panics, and `Option<SignalId>` is 8 bytes instead of 4
//
// An `Option<T>` needs somewhere to store "is it Some or None?". Usually
// that's an extra tag (plus padding). But if `T` has an *invalid bit
// pattern* (a "niche"), the compiler uses that pattern to mean `None`, and
// the tag disappears:
//   - `&T` and `Box<T>` are never null, so `Option<&T>` is one pointer.
//   - `NonZeroU32` is never 0, so `Option<NonZeroU32>` is 4 bytes.
//
// YOUR TASK
//   1. Change `SignalId` to wrap `NonZeroU32` instead of `u32`.
//   2. Implement `SignalId::new` (0 → None) and `SignalId::get`.
//   3. Implement `first_free_id`.
//   Do not edit the tests.

// I AM NOT DONE

use std::mem::size_of;
use std::num::NonZeroU32;

/// A signal id. Ids start at 1; 0 means "no signal" and must be unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SignalId(u32);

impl SignalId {
    /// Returns `None` for 0, which is never a valid id.
    fn new(id: u32) -> Option<Self> {
        todo!()
    }

    fn get(self) -> u32 {
        todo!()
    }
}

/// Returns the smallest id in `1..` that is not in `used`.
fn first_free_id(used: &[SignalId]) -> SignalId {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_not_an_id() {
        assert_eq!(SignalId::new(0), None);
        assert_eq!(SignalId::new(7).map(SignalId::get), Some(7));
    }

    #[test]
    fn option_of_signal_id_is_free() {
        assert_eq!(size_of::<SignalId>(), 4);
        assert_eq!(size_of::<Option<SignalId>>(), 4, "give SignalId a niche");
    }

    #[test]
    fn pointers_have_niches_too() {
        // These always hold: references and Box are never null.
        assert_eq!(size_of::<Option<&u64>>(), size_of::<&u64>());
        assert_eq!(size_of::<Option<Box<u64>>>(), size_of::<Box<u64>>());
        // A plain u32 has no invalid pattern, so Option needs a tag + padding.
        assert_eq!(size_of::<Option<u32>>(), 8);
    }

    #[test]
    fn first_free_skips_used_ids() {
        let used: Vec<SignalId> = [1, 2, 4].into_iter().filter_map(SignalId::new).collect();
        assert_eq!(first_free_id(&used).get(), 3);
        assert_eq!(first_free_id(&[]).get(), 1);
    }
}
