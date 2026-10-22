//! A minimal arena, like the `la-arena` crate rust-analyzer uses. It's ours
//! so that Salsa's `SalsaValue` can be implemented for it.

use std::{fmt, marker::PhantomData, ops};

/// An index into an [`Arena`] or one of the lists of an item tree.
pub struct Idx<T> {
    raw: u32,
    _ty: PhantomData<fn() -> T>,
}

impl<T> Idx<T> {
    pub(crate) fn new(raw: usize) -> Self {
        Idx { raw: raw as u32, _ty: PhantomData }
    }

    pub fn index(self) -> usize {
        self.raw as usize
    }
}

impl<T> Clone for Idx<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Idx<T> {}

impl<T> PartialEq for Idx<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Eq for Idx<T> {}

impl<T> PartialOrd for Idx<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Idx<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<T> std::hash::Hash for Idx<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> fmt::Debug for Idx<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Idx::<{}>({})", std::any::type_name::<T>().rsplit("::").next().unwrap(), self.raw)
    }
}

// SAFETY: `Idx` holds no references; the type parameter is only a marker.
unsafe impl<T> salsa::SalsaValue for Idx<T> {}

/// A list of values, addressed by [`Idx`].
#[derive(Clone, PartialEq, Eq)]
pub struct Arena<T> {
    data: Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena { data: Vec::new() }
    }
}

impl<T> Arena<T> {
    pub fn alloc(&mut self, value: T) -> Idx<T> {
        self.data.push(value);
        Idx::new(self.data.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Idx<T>, &T)> {
        self.data.iter().enumerate().map(|(i, value)| (Idx::new(i), value))
    }
}

impl<T> ops::Index<Idx<T>> for Arena<T> {
    type Output = T;
    fn index(&self, idx: Idx<T>) -> &T {
        &self.data[idx.index()]
    }
}

impl<T> ops::IndexMut<Idx<T>> for Arena<T> {
    fn index_mut(&mut self, idx: Idx<T>) -> &mut T {
        &mut self.data[idx.index()]
    }
}

impl<T: fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.data.iter().enumerate()).finish()
    }
}

// SAFETY: An arena only holds its values.
unsafe impl<T: salsa::SalsaValue> salsa::SalsaValue for Arena<T> {}

/// A map from the indices of an arena to values, like `la_arena::ArenaMap`.
#[derive(Clone, PartialEq, Eq)]
pub struct ArenaMap<K, V> {
    data: Vec<Option<V>>,
    _ty: PhantomData<fn() -> K>,
}

impl<K, V> Default for ArenaMap<K, V> {
    fn default() -> Self {
        ArenaMap { data: Vec::new(), _ty: PhantomData }
    }
}

impl<K, V> ArenaMap<K, V> {
    pub fn insert(&mut self, idx: Idx<K>, value: V) {
        let i = idx.index();
        if self.data.len() <= i {
            self.data.resize_with(i + 1, || None);
        }
        self.data[i] = Some(value);
    }

    pub fn get(&self, idx: Idx<K>) -> Option<&V> {
        self.data.get(idx.index()).and_then(Option::as_ref)
    }
}

impl<K, V: fmt::Debug> fmt::Debug for ArenaMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.data.iter().enumerate().filter_map(|(i, v)| Some((i, v.as_ref()?)))).finish()
    }
}

// SAFETY: An arena map only holds its values.
unsafe impl<K, V: salsa::SalsaValue> salsa::SalsaValue for ArenaMap<K, V> {}
