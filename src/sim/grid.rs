//! The map is unbounded: a tile's index is its position packed into a number, so it never changes
//! as the factory grows, and a floor stores only the tiles that hold something.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::{Index, IndexMut};

/// How far out the map goes from (0, 0) in every direction. Far beyond anything a factory needs;
/// it is the limit of the index, not a wall the player is likely to meet.
pub const EDGE: i32 = 32_768;

pub fn in_bounds(x: i32, y: i32) -> bool {
    (-EDGE..EDGE).contains(&x) && (-EDGE..EDGE).contains(&y)
}

/// The tile index of (x, y): rows count from the top of the map, so indexes sort in reading order.
pub fn idx(x: i32, y: i32) -> usize {
    (((y + EDGE) as usize) << 16) | (x + EDGE) as usize
}

pub fn pos_of(i: usize) -> (i32, i32) {
    ((i & 0xFFFF) as i32 - EDGE, (i >> 16) as i32 - EDGE)
}

/// Tile indexes are well spread already; this only mixes them.
#[derive(Default, Clone, Copy)]
pub struct IndexHasher(u64);

impl Hasher for IndexHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(5) ^ *b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_usize(&mut self, n: usize) {
        self.0 = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
}

type Map<T> = HashMap<usize, T, BuildHasherDefault<IndexHasher>>;

/// A value for every tile of the map that only costs memory where it differs from the default.
/// Reading a tile nobody has set gives the default; writing one stores it.
#[derive(Clone, Debug)]
pub struct Sparse<T> {
    map: Map<T>,
    default: T,
}

impl<T: Clone> Sparse<T> {
    pub fn new(default: T) -> Self {
        Sparse { map: Map::default(), default }
    }

    pub fn default_value(&self) -> &T {
        &self.default
    }

    /// Every written tile with its index, in index order.
    pub fn entries(&self) -> Vec<(usize, &T)> {
        let mut v: Vec<(usize, &T)> = self.map.iter().map(|(k, v)| (*k, v)).collect();
        v.sort_unstable_by_key(|e| e.0);
        v
    }

    pub fn written(&self) -> usize {
        self.map.len()
    }
}

impl<T: Clone + PartialEq> PartialEq for Sparse<T> {
    /// Equal when every tile reads the same, however each got there.
    fn eq(&self, other: &Self) -> bool {
        self.default == other.default && self.map.keys().all(|k| self[*k] == other[*k]) && other.map.keys().all(|k| self[*k] == other[*k])
    }
}

impl<T: Clone> Index<usize> for Sparse<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        self.map.get(&i).unwrap_or(&self.default)
    }
}

impl<T: Clone> IndexMut<usize> for Sparse<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        let default = &self.default;
        self.map.entry(i).or_insert_with(|| default.clone())
    }
}

/// For maps of `Option`s (buildings, owners): only the occupied tiles.
impl<T: Clone> Sparse<Option<T>> {
    /// The occupied tiles, in index order, as `&Option` so `.flatten()` reads them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn iter(&self) -> impl Iterator<Item = &Option<T>> {
        self.entries().into_iter().filter(|(_, v)| v.is_some()).map(|(_, v)| v)
    }

    /// The occupied tiles, in no particular order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<T>> {
        self.map.values_mut().filter(|v| v.is_some())
    }

    /// The occupied tiles with their indexes, in index order.
    pub fn occupied(&self) -> Vec<(usize, &T)> {
        self.entries().into_iter().filter_map(|(i, v)| v.as_ref().map(|b| (i, b))).collect()
    }

    /// The occupied tiles with their indexes, in index order, to change.
    pub fn occupied_mut(&mut self) -> Vec<(usize, &mut T)> {
        let mut v: Vec<(usize, &mut T)> = self.map.iter_mut().filter_map(|(k, v)| v.as_mut().map(|b| (*k, b))).collect();
        v.sort_unstable_by_key(|e| e.0);
        v
    }

    /// Drops the empty entries that reading-for-writing leaves behind.
    pub fn prune(&mut self) {
        self.map.retain(|_, v| v.is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_round_trip_and_sort_in_reading_order() {
        for (x, y) in [(0, 0), (-5, 7), (-EDGE, -EDGE), (EDGE - 1, EDGE - 1), (100, -3000)] {
            assert_eq!(pos_of(idx(x, y)), (x, y));
        }
        assert!(idx(5, 0) < idx(6, 0));
        assert!(idx(EDGE - 1, 0) < idx(-EDGE, 1), "the next row comes after the whole row above");
        assert!(in_bounds(-EDGE, EDGE - 1) && !in_bounds(EDGE, 0) && !in_bounds(0, -EDGE - 1));
    }

    #[test]
    fn a_sparse_map_reads_its_default_and_stores_only_what_is_written() {
        let mut s: Sparse<Option<u32>> = Sparse::new(None);
        assert_eq!(s[idx(3, 4)], None);
        assert_eq!(s.written(), 0);
        s[idx(3, 4)] = Some(9);
        s[idx(-1, 4)] = Some(5);
        assert_eq!(s[idx(3, 4)], Some(9));
        assert_eq!(s.iter().flatten().copied().collect::<Vec<_>>(), vec![5, 9], "index order, not insertion order");
        s[idx(3, 4)] = None;
        s.prune();
        assert_eq!(s.written(), 1);
    }
}
