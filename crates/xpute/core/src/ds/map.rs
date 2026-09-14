// xpute-core/ds/map.rs

//! A dense array of keys with an index into it: insert appends, delete moves
//! the last entry into the hole, and a walk over every entry is a walk over
//! contiguous memory.
//!
//! Laid out as lanes. The keys are one typed array in position order, and the
//! index from key to position is an open-addressing table of positions —
//! linear probing, deletion by backward shift, so no tombstones pile up. There
//! is no object per entry. An earlier version kept `[K, V]` tuples and a JS
//! Map: dense in order, boxed in every other respect, and neither half could
//! have followed the data into linear memory.
//!
//! Values are not stored. A position is the value: `insert` returns it,
//! `find` looks it up, and `remove_at` says which entry moved into a freed
//! position — a caller keeps what it stores per entry in its own lanes at the
//! same position and moves them along.
//!
//! Keys are integers of one width, chosen by KeyTraits: I64_KEYS for 64-bit
//! keys (a packed IVec, say), U32_KEYS for 32-bit ones. The two cannot share
//! one lane type — typed arrays split into a number family and a BigInt family
//! (kit/array.rs) — so the traits carry the lane, the hash and the canonical
//! form, the same way sparse_tree takes its key operations as PVecTraits.
//! The lane's constructor is the key type itself here.

use crate::kit::array::{AnyElement, TypedArray};

use super::typed_alloc::{TypedAlloc, HEAP_ALLOC};

pub trait KeyTraits {
    /// The key, and the lane it is stored in.
    type Key: AnyElement + PartialEq;
    /// The form a key is stored and compared in.
    fn norm(&self, key: Self::Key) -> Self::Key;
    /// A 32-bit mix whose high bits are well spread; the table takes a slot
    /// from the top bits.
    fn hash(&self, key: Self::Key) -> u32;
}

/// 64-bit keys, compared as signed: a key past 2^63 — an unsigned packed
/// word — folds with asIntN on the way in, so the same number always finds
/// the same entry; here the fold is the cast to i64, and `norm` is the
/// identity it leaves. `key_at` returns the folded form.
pub struct I64Keys;

impl KeyTraits for I64Keys {
    type Key = i64;
    fn norm(&self, k: i64) -> i64 {
        k
    }
    // Fibonacci hashing: the high half of k · 2^64/φ.
    fn hash(&self, k: i64) -> u32 {
        ((k as u64).wrapping_mul(0x9e3779b97f4a7c15) >> 32) as u32
    }
}

pub const I64_KEYS: I64Keys = I64Keys;

pub struct U32Keys;

impl KeyTraits for U32Keys {
    type Key = u32;
    fn norm(&self, k: u32) -> u32 {
        k
    }
    fn hash(&self, k: u32) -> u32 {
        k.wrapping_mul(0x9e3779b1)
    }
}

pub const U32_KEYS: U32Keys = U32Keys;

pub struct IndexedMap<T: KeyTraits> {
    traits: T,
    alloc: Box<dyn TypedAlloc>,
    keys: TypedArray<T::Key>,
    /// Per slot, position + 1; 0 is an empty slot.
    table: TypedArray<u32>,
    /// 32 − log2(table size): a hash's top bits pick the home slot.
    shift: u32,
    mask: u32,
    _size: u32,
}

impl<T: KeyTraits> IndexedMap<T> {
    pub fn new(traits: T, cap: u32, alloc: Option<Box<dyn TypedAlloc>>) -> IndexedMap<T> {
        let mut alloc = alloc.unwrap_or_else(|| Box::new(HEAP_ALLOC));
        let mut size: u32 = 2;
        while size < cap * 2 {
            size <<= 1;
        }
        let table = TypedArray::of(&alloc.take_view(u32::KIND, size)).unwrap();
        let keys = TypedArray::of(&alloc.take_view(T::Key::KIND, size >> 1)).unwrap();
        IndexedMap {
            traits,
            alloc,
            keys,
            table,
            mask: size - 1,
            shift: 32 - size.ilog2(),
            _size: 0,
        }
    }

    pub fn size(&self) -> u32 {
        self._size
    }

    /// The key at a position, in its stored form.
    pub fn key_at(&self, pos: u32) -> T::Key {
        self.keys.get(pos as usize)
    }

    /// The key's position, or -1.
    pub fn find(&self, key: T::Key) -> i32 {
        let k = self.traits.norm(key);
        let mut s = self.home(k);
        loop {
            let e = self.table.get(s as usize);
            if e == 0 {
                return -1;
            }
            if self.keys.get(e as usize - 1) == k {
                return e as i32 - 1;
            }
            s = (s + 1) & self.mask;
        }
    }

    /// The key's position, appending it at the end if it is new.
    pub fn insert(&mut self, key: T::Key) -> u32 {
        let found = self.find(key);
        if found >= 0 {
            return found as u32;
        }
        if (self._size + 1) * 2 > self.table.length() as u32 {
            self.grow();
        }
        let k = self.traits.norm(key);
        let pos = self._size;
        self._size += 1;
        self.keys.put(pos as usize, k);
        self.place(k, pos);
        pos
    }

    /// Removes the entry at `pos` by moving the last entry into it. Returns the
    /// position that entry came from — move your own lanes from there to `pos` —
    /// or `pos` itself when the removed entry was the last.
    pub fn remove_at(&mut self, pos: u32) -> u32 {
        let slot = self.slot_of(self.keys.get(pos as usize));
        self.unplace(slot);
        self._size -= 1;
        let last = self._size;
        if pos != last {
            let moved = self.keys.get(last as usize);
            let slot = self.slot_of(moved);
            self.table.put(slot as usize, pos + 1);
            self.keys.put(pos as usize, moved);
        }
        last
    }

    /// Forgets every entry; the capacity stays.
    pub fn clear(&mut self) {
        self.table.fill(0);
        self._size = 0;
    }

    /// Every position once, in an order fixed by `seed`, in O(1) memory: a
    /// start and a step coprime to the size walk the positions as a cycle. For
    /// spreading work or routing without favouring insertion order. Mutating
    /// during the walk is undefined.
    pub fn positions_seeded(&self, seed: u64) -> impl Iterator<Item = u32> {
        let n = self._size;
        let s = splitmix64(seed);
        let start = if n == 0 { 0 } else { (s % n as u64) as u32 };
        let step = if n == 0 { 1 } else { coprime_step(n, splitmix64(s ^ 0x9e3779b97f4a7c15)) };
        let mut pos = start;
        (0..n).map(move |_| {
            let out = pos;
            pos = (pos + step) % n;
            out
        })
    }

    fn take_keys(&mut self, n: u32) -> TypedArray<T::Key> {
        TypedArray::of(&self.alloc.take_view(T::Key::KIND, n)).unwrap()
    }

    fn home(&self, k: T::Key) -> u32 {
        self.traits.hash(k) >> self.shift
    }

    fn place(&mut self, k: T::Key, pos: u32) {
        let mut s = self.home(k);
        while self.table.get(s as usize) != 0 {
            s = (s + 1) & self.mask;
        }
        self.table.put(s as usize, pos + 1);
    }

    fn slot_of(&self, k: T::Key) -> u32 {
        let mut s = self.home(k);
        while self.keys.get(self.table.get(s as usize) as usize - 1) != k {
            s = (s + 1) & self.mask;
        }
        s
    }

    /// Backward-shift deletion: pull later entries of the probe run into the
    /// hole unless their own home lies cyclically between the hole and them.
    fn unplace(&mut self, hole: u32) {
        let mut i = hole;
        let mut j = hole;
        self.table.put(i as usize, 0);
        loop {
            j = (j + 1) & self.mask;
            let e = self.table.get(j as usize);
            if e == 0 {
                return;
            }
            let h = self.home(self.keys.get(e as usize - 1));
            let stays = if i <= j { h > i && h <= j } else { h > i || h <= j };
            if !stays {
                self.table.put(i as usize, e);
                self.table.put(j as usize, 0);
                i = j;
            }
        }
    }

    fn grow(&mut self) {
        let size = self.table.length() as u32 * 2;
        let keys = self.take_keys(size >> 1);
        keys.set(&self.keys.subarray(0, self._size as usize));
        self.alloc.give(&self.keys.view());
        self.alloc.give(&self.table.view());
        self.keys = keys;
        self.table = TypedArray::of(&self.alloc.take_view(u32::KIND, size)).unwrap();
        self.mask = size - 1;
        self.shift = 32 - size.ilog2();
        for pos in 0..self._size {
            self.place(self.keys.get(pos as usize), pos);
        }
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn coprime_step(n: u32, seed: u64) -> u32 {
    if n == 1 {
        return 1;
    }
    let mut step = (seed % n as u64) as u32;
    if step == 0 {
        step = 1;
    }
    step = (step | 1) % n;
    if step == 0 {
        step = 1;
    }
    while gcd(step, n) != 1 {
        step = (step + 2) % n;
        if step == 0 {
            step = 1;
        }
    }
    step
}

/// SplitMix64.
fn splitmix64(x: u64) -> u64 {
    let x = x.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
#[path = "map.test.rs"]
mod test;
