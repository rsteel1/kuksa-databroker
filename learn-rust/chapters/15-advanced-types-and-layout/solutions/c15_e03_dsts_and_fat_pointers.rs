// c15_e03_dsts_and_fat_pointers.rs
//
// CONCEPT: dynamically sized types (`str`, `[T]`, `dyn Trait`), fat pointers, `?Sized`
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Dynamically sized types and fat pointers")
// DATABROKER: databroker/src/broker.rs — `get_entry_by_path(&self, path: impl AsRef<str>)`
//             and `Arc<dyn SignalProvider>`: both are references/pointers to unsized data.
// EXPECT:  compile error E0277 (the size for values of type `str` cannot be known
//          at compilation time)
//
// `str`, `[T]` and `dyn Trait` have no size known at compile time, so you can
// only use them behind a pointer (`&str`, `Box<[T]>`, `Arc<dyn Trait>`). That
// pointer is "fat": two words, the address plus a length (for `str`/`[T]`)
// or a vtable pointer (for `dyn Trait`).
//
// Every generic parameter has an invisible `T: Sized` bound. `?Sized` removes
// it ("T may or may not be sized"), which is fine as long as you only touch
// `T` through a pointer, as in `&T`.
//
// YOUR TASK
//   Make `report` accept unsized types by relaxing its bound. Don't change
//   the trait impls or the tests.

use std::mem::size_of;

trait HasLen {
    fn len_units(&self) -> usize;
}

impl HasLen for str {
    fn len_units(&self) -> usize {
        self.len()
    }
}

impl<T> HasLen for [T] {
    fn len_units(&self) -> usize {
        self.len()
    }
}

impl<T> HasLen for Vec<T> {
    fn len_units(&self) -> usize {
        self.len()
    }
}

// WHY: `?Sized` because `T` is only ever used behind `&`.
fn report<T: HasLen + ?Sized>(item: &T) -> String {
    format!("{} units", item.len_units())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORD: usize = size_of::<usize>();

    #[test]
    fn works_for_str_and_slices() {
        assert_eq!(report("Vehicle.Speed"), "13 units");
        let ids = [1, 2, 3];
        assert_eq!(report(&ids[..]), "3 units");
    }

    #[test]
    fn works_for_sized_types_too() {
        assert_eq!(report(&vec![1u8, 2]), "2 units");
    }

    #[test]
    fn works_for_trait_objects() {
        let boxed: Box<dyn HasLen> = Box::new(vec!['a', 'b', 'c', 'd']);
        assert_eq!(report(boxed.as_ref()), "4 units");
    }

    #[test]
    fn fat_pointers_are_two_words() {
        assert_eq!(size_of::<&u8>(), WORD); // thin: just an address
        assert_eq!(size_of::<&str>(), 2 * WORD); // address + length
        assert_eq!(size_of::<&[u64]>(), 2 * WORD); // address + length
        assert_eq!(size_of::<&dyn HasLen>(), 2 * WORD); // address + vtable
        assert_eq!(size_of::<Box<str>>(), 2 * WORD);
        assert_eq!(size_of::<String>(), 3 * WORD); // pointer + length + capacity
    }
}
