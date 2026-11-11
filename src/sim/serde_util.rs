//! Save helpers: per-item tables and flag sets that are written by NAME, never by position,
//! so adding or reordering items, companies or techs never changes what an old save means.

use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut, Index, IndexMut};

use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::content::{Item, ITEMS};

/// One value per item. At run time it is an array indexed by `Item::idx()` (so `t[item.idx()]`,
/// `t[item]` and the slice methods all work), kept in a box so the structs holding one stay small.
/// In a save it is a map of item name to value, with the default values left out.
#[derive(Clone, Debug, PartialEq)]
pub struct PerItem<T>(Box<[T; ITEMS]>);

impl<T: Default> Default for PerItem<T> {
    fn default() -> Self {
        PerItem(Box::new(std::array::from_fn(|_| T::default())))
    }
}

impl<T> Deref for PerItem<T> {
    type Target = [T; ITEMS];
    fn deref(&self) -> &[T; ITEMS] {
        &self.0
    }
}

impl<T> DerefMut for PerItem<T> {
    fn deref_mut(&mut self) -> &mut [T; ITEMS] {
        &mut self.0
    }
}

impl<T> Index<usize> for PerItem<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.0[i]
    }
}

impl<T> IndexMut<usize> for PerItem<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.0[i]
    }
}

impl<T> Index<Item> for PerItem<T> {
    type Output = T;
    fn index(&self, i: Item) -> &T {
        &self.0[i.idx()]
    }
}

impl<T> IndexMut<Item> for PerItem<T> {
    fn index_mut(&mut self, i: Item) -> &mut T {
        &mut self.0[i.idx()]
    }
}

impl<T: Copy> PerItem<T> {
    /// A copy of the table as a plain array.
    pub fn to_array(&self) -> [T; ITEMS] {
        *self.0
    }
}

/// The name an enum variant is saved under (what serde derives for a unit variant).
fn variant_name<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}")
}

/// The entry of `all` whose variant name is `name`.
pub fn by_name<T: std::fmt::Debug + Copy>(all: &[T], name: &str) -> Option<T> {
    all.iter().copied().find(|v| variant_name(v) == name)
}

impl<T: Serialize + Default + PartialEq> Serialize for PerItem<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let zero = T::default();
        let mut map = s.serialize_map(None)?;
        for it in Item::ALL {
            let v = &self.0[it.idx()];
            if *v != zero {
                map.serialize_entry(&variant_name(&it), v)?;
            }
        }
        map.end()
    }
}

impl<'de, T: Deserialize<'de> + Default> Deserialize<'de> for PerItem<T> {
    /// A name that isn't an item is an error: the save is from a newer game, and dropping
    /// its numbers silently would lose them.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = BTreeMap::<String, T>::deserialize(d)?;
        let mut out = PerItem::default();
        for (name, v) in map {
            let Some(item) = by_name(&Item::ALL, &name) else {
                return Err(D::Error::custom(format!("unknown item '{name}' (a save from a newer version?)")));
            };
            out[item] = v;
        }
        Ok(out)
    }
}

/// Writes the true entries of a flag array as a list of names: `["Hollowmere", ...]`.
/// `all` lists every value and `idx` says where each one's flag is.
pub fn ser_set<S, T, const N: usize>(flags: &[bool; N], all: &[T; N], idx: fn(T) -> usize, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: std::fmt::Debug + Copy,
{
    s.collect_seq(all.iter().filter(|v| flags[idx(**v)]).map(variant_name))
}

/// Reads a list of names into a flag array. An unknown name is an error (a newer save).
pub fn de_set<'de, D, T, const N: usize>(d: D, all: &[T; N], idx: fn(T) -> usize, what: &str) -> Result<[bool; N], D::Error>
where
    D: Deserializer<'de>,
    T: std::fmt::Debug + Copy,
{
    let names = Vec::<String>::deserialize(d)?;
    let mut flags = [false; N];
    for name in names {
        let Some(v) = by_name(all, &name) else {
            return Err(D::Error::custom(format!("unknown {what} '{name}' (a save from a newer version?)")));
        };
        flags[idx(v)] = true;
    }
    Ok(flags)
}
