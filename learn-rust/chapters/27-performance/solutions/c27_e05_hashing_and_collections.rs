// c27_e05_hashing_and_collections.rs
//
// CONCEPT: `HashMap` costs: rehashing on growth, the default SipHash hasher
//          (DoS-resistant but slow for small keys), custom `BuildHasher`s,
//          and borrowed-key lookups that don't allocate.
// GUIDE:   chapters/27-performance/guide.md  (section "Hashing and choosing a collection")
// DATABROKER: databroker/src/broker.rs:128-132 — `Database` keeps
//             `path_to_id: HashMap<String, i32>` and `entries: HashMap<i32, Entry>`.
//             Every request resolves a path, then an id, so both lookups
//             are on the hot path. In the wild: hashbrown-0.15.5 (the table
//             behind std's HashMap) makes foldhash its default hasher
//             (Cargo.toml `default-hasher = ["dep:foldhash"]`).
// EXPECT:  tests fail: todo!() panics
//
// std's default hasher (SipHash-1-3) is deliberately hard to attack: an
// adversary can't craft keys that all collide. For small integer keys that
// *you* generate (like signal ids) a much simpler hash is fine and faster.
// Never use a weak hash on keys an attacker controls (HTTP headers, JWT
// claims, client-supplied paths).
//
// YOUR TASK
//   1. Implement `IdHasher` (a multiplicative "Fx-style" hash for integers).
//   2. Implement `index_paths` so it allocates only for the map itself:
//      pre-size it, and store `&str` keys borrowed from the input.
//   3. Implement `lookup` without allocating.
//   Do not edit the tests.

#![allow(unsafe_code)] // only for the provided counting allocator below

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

// ---------------------------------------------------------------------------
// Provided: the counting allocator you built in c27_e01. It counts every
// allocation and reallocation made *by the current thread*, so tests running
// in parallel on other threads don't disturb the numbers.
// ---------------------------------------------------------------------------
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        // `const` initialisation: accessing it never allocates, so the
        // allocator can use it without recursing into itself.
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    fn bump() {
        // `try_with` instead of `with`: never panic inside the allocator,
        // even while a thread is being torn down.
        let _ = ALLOCS.try_with(|count| count.set(count.get() + 1));
    }

    pub struct CountingAlloc;

    // SAFETY: every method forwards to `System` with the caller's arguments
    // unchanged, so all of `GlobalAlloc`'s contracts are upheld by `System`.
    // The only extra work touches a const-initialised thread-local `Cell`,
    // which neither allocates nor unwinds.
    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc`'s contract; passed through.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc_zeroed`'s contract; passed through.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `realloc`'s contract; passed through.
            unsafe { System.realloc(ptr, layout, new_size) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: `ptr` was allocated by `System` via the methods above
            // with this `layout`; the caller upholds `dealloc`'s contract.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    /// Runs `f` and returns its result plus the number of (re)allocations
    /// it performed on the current thread.
    pub fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let before = ALLOCS.with(Cell::get);
        let result = f();
        let after = ALLOCS.with(Cell::get);
        (result, after - before)
    }
}
use counting::count_allocs;

/// A tiny, fast, NON-cryptographic hasher for integer keys we generate ourselves.
#[derive(Default)]
struct IdHasher {
    hash: u64,
}

/// Large odd constant used by rustc's FxHasher.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl Hasher for IdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    fn write_u64(&mut self, n: u64) {
        // WHY: rotate, mix in the input, multiply by an odd constant, the Fx recipe.
        self.hash = (self.hash.rotate_left(5) ^ n).wrapping_mul(SEED);
    }

    fn write_i32(&mut self, n: i32) {
        self.write_u64(u64::from(n as u32));
    }

    fn finish(&self) -> u64 {
        self.hash
    }
}

type IdMap<V> = HashMap<i32, V, BuildHasherDefault<IdHasher>>;

/// Builds id -> value with the fast hasher.
fn build_id_map(values: &[(i32, f64)]) -> IdMap<f64> {
    let mut map = IdMap::with_capacity_and_hasher(values.len(), Default::default());
    map.extend(values.iter().copied());
    map
}

/// Maps each path to its position. Keys borrow from `paths`.
/// Budget: exactly one allocation (the table).
fn index_paths<'a>(paths: &[&'a str]) -> HashMap<&'a str, usize> {
    let mut map = HashMap::with_capacity(paths.len());
    for (i, path) in paths.iter().enumerate() {
        map.insert(*path, i);
    }
    map
}

/// Looks up by `&str` in a map with `String` keys. Budget: zero allocations.
fn lookup(map: &HashMap<String, i32>, path: &str) -> Option<i32> {
    // WHY: `String: Borrow<str>`, so `get` accepts `&str` directly, with no
    // temporary `String` needed.
    map.get(path).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn hash_of(id: i32) -> u64 {
        let mut h = IdHasher::default();
        id.hash(&mut h);
        h.finish()
    }

    #[test]
    fn hasher_is_deterministic_and_spreads() {
        assert_eq!(hash_of(42), hash_of(42));
        let mut hashes: Vec<u64> = (0..1000).map(hash_of).collect();
        hashes.sort_unstable();
        hashes.dedup();
        assert_eq!(hashes.len(), 1000, "no collisions among the first 1000 ids");
        let mut h = IdHasher::default();
        h.write(b"Vehicle.Speed");
        assert_ne!(h.finish(), 0);
    }

    #[test]
    fn id_map_works() {
        let map = build_id_map(&[(1, 1.5), (7, 7.5), (-3, 0.5)]);
        assert_eq!(map.get(&7), Some(&7.5));
        assert_eq!(map.get(&-3), Some(&0.5));
        assert_eq!(map.get(&2), None);
    }

    #[test]
    fn index_allocates_once() {
        let paths = ["Vehicle.Speed", "Vehicle.Cabin.Temp", "Body.Lights.IsOn"];
        let paths: Vec<&str> = paths.iter().copied().cycle().take(300).collect();
        let (map, n) = count_allocs(|| index_paths(&paths[..3]));
        assert_eq!(map["Vehicle.Cabin.Temp"], 1);
        assert_eq!(n, 1, "index_paths allocated {n} times");
        let (big, n) = count_allocs(|| index_paths(&paths));
        assert_eq!(big.len(), 3, "duplicates overwrite");
        assert_eq!(big["Body.Lights.IsOn"], 299);
        assert_eq!(n, 1, "pre-size to avoid rehashing (saw {n})");
    }

    #[test]
    fn borrowed_lookup_is_free() {
        let map: HashMap<String, i32> = [("Vehicle.Speed".to_string(), 3)].into();
        let (hit, n) = count_allocs(|| lookup(&map, "Vehicle.Speed"));
        assert_eq!(hit, Some(3));
        assert_eq!(n, 0);
        assert_eq!(lookup(&map, "nope"), None);
    }
}
