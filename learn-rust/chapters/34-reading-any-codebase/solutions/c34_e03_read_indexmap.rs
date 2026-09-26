// c34_e03_read_indexmap.rs
//
// CONCEPT: read-then-rebuild `indexmap`: a hash map that remembers insertion
//          order by storing entries densely in a `Vec` and keeping a hash
//          table of positions, and the two removal strategies that design
//          forces (O(1) `swap_remove` vs order-keeping O(n) `shift_remove`).
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Reading data-structure code")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` builds an
//             `IndexMap<SignalId, Datapoint>` so responses come back in request order.
// EXPECT:  tests fail: `todo!()` panics
//
// READ FIRST (indexmap 2.14.2, under ~/.cargo/registry/src/index.crates.io-*/indexmap-2.14.2/):
//   src/map.rs:93     `pub struct IndexMap<K, V, S>` — just `core` + a hasher
//   src/inner.rs:29   `struct Core { indices, entries }` — the whole idea
//   src/lib.rs:147    `struct Bucket { hash, key, value }` — one entry
//   src/inner.rs:564  `shift_remove_finish` — `Vec::remove` + fix every later index
//   src/inner.rs:587  `swap_remove_finish` — `Vec::swap_remove` + fix ONE index
//   src/map.rs:1037   the public `swap_remove_full`
// The real `indices` is a `hashbrown::HashTable<usize>` that stores only
// positions and looks keys up through `entries`, so keys aren't stored twice.
// You'll use a `HashMap<K, usize>` (keys cloned): same behaviour, simpler.
//
// YOUR TASK
//   Implement the `todo!()`s. The last test runs one script of operations on
//   your map and on the real `IndexMap` and compares the results after every
//   step. Do not edit the tests.

use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct OrderedMap<K, V> {
    /// Entries in insertion order. This is the source of truth for order.
    entries: Vec<(K, V)>,
    /// key → position in `entries`. Must always agree with `entries`.
    indices: HashMap<K, usize>,
}

impl<K: Hash + Eq + Clone, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        OrderedMap {
            entries: Vec::new(),
            indices: HashMap::new(),
        }
    }
}

impl<K: Hash + Eq + Clone, V> OrderedMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts or replaces. A replaced key KEEPS its original position.
    /// Returns `(position, old value)` like `IndexMap::insert_full`.
    pub fn insert_full(&mut self, key: K, value: V) -> (usize, Option<V>) {
        match self.indices.get(&key) {
            Some(&i) => {
                let old = std::mem::replace(&mut self.entries[i].1, value);
                (i, Some(old))
            }
            None => {
                let i = self.entries.len();
                self.indices.insert(key.clone(), i);
                self.entries.push((key, value));
                (i, None)
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let &i = self.indices.get(key)?;
        Some(&self.entries[i].1)
    }

    pub fn get_index_of(&self, key: &K) -> Option<usize> {
        self.indices.get(key).copied()
    }

    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.entries.get(index).map(|(k, v)| (k, v))
    }

    /// O(1): the LAST entry moves into the hole, so order changes.
    pub fn swap_remove(&mut self, key: &K) -> Option<V> {
        let i = self.indices.remove(key)?;
        let (_, value) = self.entries.swap_remove(i);
        // WHY: exactly one other entry moved (the old last one, now at `i`),
        // so exactly one index needs fixing. Same as inner.rs:587.
        if let Some((moved_key, _)) = self.entries.get(i) {
            self.indices.insert(moved_key.clone(), i);
        }
        Some(value)
    }

    /// O(n): later entries shift left by one, so order is preserved.
    pub fn shift_remove(&mut self, key: &K) -> Option<V> {
        let i = self.indices.remove(key)?;
        let (_, value) = self.entries.remove(i);
        // WHY: every entry after `i` moved one place left.
        for (k, _) in &self.entries[i..] {
            *self.indices.get_mut(k).expect("every entry has an index") -= 1;
        }
        Some(value)
    }

    /// Entries in order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    fn keys(map: &OrderedMap<&'static str, i32>) -> Vec<&'static str> {
        map.iter().map(|(k, _)| *k).collect()
    }

    fn filled() -> OrderedMap<&'static str, i32> {
        let mut map = OrderedMap::new();
        for (i, key) in ["a", "b", "c", "d", "e"].into_iter().enumerate() {
            map.insert_full(key, i as i32);
        }
        map
    }

    #[test]
    fn keeps_insertion_order_and_positions() {
        let mut map = filled();
        assert_eq!(keys(&map), ["a", "b", "c", "d", "e"]);
        assert_eq!(
            map.insert_full("c", 30),
            (2, Some(2)),
            "replace keeps position"
        );
        assert_eq!(map.insert_full("f", 5), (5, None));
        assert_eq!(map.get(&"c"), Some(&30));
        assert_eq!(map.get_index_of(&"f"), Some(5));
        assert_eq!(map.get_index(0), Some((&"a", &0)));
        assert_eq!(map.get(&"zzz"), None);
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn swap_remove_moves_the_last_entry_into_the_hole() {
        let mut map = filled();
        assert_eq!(map.swap_remove(&"b"), Some(1));
        assert_eq!(keys(&map), ["a", "e", "c", "d"]);
        assert_eq!(map.get_index_of(&"e"), Some(1));
        assert_eq!(map.swap_remove(&"d"), Some(3), "removing the last entry");
        assert_eq!(keys(&map), ["a", "e", "c"]);
        assert_eq!(map.swap_remove(&"b"), None);
    }

    #[test]
    fn shift_remove_preserves_order() {
        let mut map = filled();
        assert_eq!(map.shift_remove(&"b"), Some(1));
        assert_eq!(keys(&map), ["a", "c", "d", "e"]);
        for (i, key) in ["a", "c", "d", "e"].iter().enumerate() {
            assert_eq!(map.get_index_of(key), Some(i), "index of {key}");
        }
        assert!(!map.is_empty());
    }

    #[derive(Debug, Clone, Copy)]
    enum Op {
        Insert(u8, i32),
        SwapRemove(u8),
        ShiftRemove(u8),
    }

    #[test]
    fn matches_the_real_indexmap_step_by_step() {
        let mut ops = Vec::new();
        // A deterministic pseudo-random script (no rand crate needed).
        let mut seed: u32 = 12345;
        for step in 0..400 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let key = ((seed >> 16) % 12) as u8;
            ops.push(match (seed >> 8) % 4 {
                0 | 1 => Op::Insert(key, step),
                2 => Op::SwapRemove(key),
                _ => Op::ShiftRemove(key),
            });
        }

        let mut mine: OrderedMap<u8, i32> = OrderedMap::new();
        let mut real: IndexMap<u8, i32> = IndexMap::new();
        for op in ops {
            match op {
                Op::Insert(k, v) => assert_eq!(mine.insert_full(k, v), real.insert_full(k, v)),
                Op::SwapRemove(k) => assert_eq!(mine.swap_remove(&k), real.swap_remove(&k)),
                Op::ShiftRemove(k) => assert_eq!(mine.shift_remove(&k), real.shift_remove(&k)),
            }
            let mine_order: Vec<(u8, i32)> = mine.iter().map(|(k, v)| (*k, *v)).collect();
            let real_order: Vec<(u8, i32)> = real.iter().map(|(k, v)| (*k, *v)).collect();
            assert_eq!(mine_order, real_order, "after {op:?}");
            for k in 0..12 {
                assert_eq!(mine.get_index_of(&k), real.get_index_of(&k), "index of {k}");
            }
        }
    }
}
