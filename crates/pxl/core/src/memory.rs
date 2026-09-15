// pxl_core/memory.rs

//! The kernel's policy over the memory the central scheduler gives it: which
//! sections there are and how many regions each takes. Cutting the memory
//! into sections is the mechanism's (xpute_core::sys::memory); what a
//! section holds, and what "full" means in it, is decided here
//! (issue/20260913-sections-of-the-core-memory-and-what-full-means.md).
//!
//! Sections, in address order:
//!
//!   module   the module's stack and data, where a wasm32 linker puts them
//!            (`--stack-first`, then data), and its heap for as long as the
//!            kernel still has one.
//!   fixed    what is allocated once and never freed: the change log, the
//!            scheduler's lanes, the volumes' standing band stores. Refuses
//!            rather than spills, so a shortfall fails at boot.
//!   page     resident terrain and POI pages, bounded by the count kept.
//!   mesh     chunk meshes, run sets, tier caches, bands — what churns as the
//!            view moves, and what the reclaim pass answers to.

use core::fmt::{self, Write};
use core::mem::MaybeUninit;

use xpute_core::ds::typed_alloc::TypedAlloc;
use xpute_core::kit::array::{ArrayBuffer, ArrayBufferView, Kind};
use xpute_core::sys::memory::{lay_out as cut, Section};

use crate::global::Global;

/// The unit sections are counted in, and a page or mesh pool's largest block.
pub const REGION_BYTES: u32 = 1 << 26;

/// The module's own section, from address 0.
pub const MODULE_BYTES: u32 = 32 << 20;

/// The fixed section's pool cuts what is left of the first region in 16 MB
/// regions, so the map stays region-aligned where the pools need it.
pub const FIXED_REGION_BYTES: u32 = 16 << 20;

/// The smallest block a section's pool hands out.
pub const MIN_BLOCK_BYTES: u32 = 1 << 12;

/// The submission and completion rings, at fixed places the host finds from
/// the boot block, and the completion ring's spill, which only the kernel
/// reads: the first three blocks of the fixed section, taken at boot before
/// anything else and never given back.
pub const RING_BYTES: u32 = 1 << 20;
pub const RING_SQ_AT: u32 = MODULE_BYTES;
pub const RING_CQ_AT: u32 = MODULE_BYTES + RING_BYTES;
pub const RING_SPILL_AT: u32 = MODULE_BYTES + 2 * RING_BYTES;

/// The sections' pools, as the kernel holds them.
pub const FIXED_POOL: u32 = 0;
pub const PAGE_POOL: u32 = 1;
pub const MESH_POOL: u32 = 2;

/// The most regions the page section takes: three hold every page kept
/// resident (module/world/terrain/config.rs derives the count from this
/// section); past them, every region is the mesh section's churn.
const PAGE_REGIONS_MAX: u32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub page: Section,
    pub mesh: Section,
}

/// The map over a memory of `bytes`: one fixed region, the page section up to
/// PAGE_REGIONS_MAX with a region left over, and the rest mesh — eight
/// regions are 1, 3 and 4, four are 1, 2 and 1.
pub fn lay_out(bytes: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    assert!(regions >= 3, "the kernel needs a region for each section");
    let page = (regions - 2).min(PAGE_REGIONS_MAX);
    let mut s = [Section::default(); 3];
    cut(bytes, MODULE_BYTES, REGION_BYTES, FIXED_REGION_BYTES, &[1, page], &mut s);
    Map {
        fixed: s[0],
        page: s[1],
        mesh: s[2],
    }
}

static MAP: Global<Option<Map>> = Global::new();

/// Records the map the module laid its memory out in.
pub fn set_map(map: Map) {
    MAP.set(Some(map));
}

/// The map, once the module has laid its memory out; none under a native
/// test, which has no sections.
pub fn map() -> Option<Map> {
    *MAP.get(|| None)
}

/// Room for `count` values of `T` in the fixed section, taken once and never
/// given back. A native test has no sections, and is handed memory of its own.
fn fixed_lane<T>(count: usize) -> &'static mut [MaybeUninit<T>] {
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = (count * core::mem::size_of::<T>()).max(1) as u32;
        let at = crate::kernel::kernel().pools[FIXED_POOL as usize].acquire(bytes).expect("the fixed section has no room for a lane");
        assert!(at.is_multiple_of(core::mem::align_of::<T>()), "a pool block is not aligned for its lane");
        // SAFETY: a block of the fixed section no one else holds, `count`
        // values long; MaybeUninit asks nothing of the bytes.
        unsafe { core::slice::from_raw_parts_mut(at as *mut MaybeUninit<T>, count) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Box::leak((0..count).map(|_| MaybeUninit::uninit()).collect::<Vec<_>>().into_boxed_slice())
    }
}

/// The module's memory as one buffer, which every fixed view shares: a view
/// is then a clone of one handle and an offset, and taking one asks nothing
/// of the heap past the first.
#[cfg(target_arch = "wasm32")]
static MEMORY: Global<ArrayBuffer> = Global::new();

/// The xpute structures' allocator over the fixed section, for one kept for
/// the kernel's life. Nothing is given back, so a structure that grows
/// through its allocator has to be sized never to.
pub struct FixedAlloc;

impl TypedAlloc for FixedAlloc {
    fn take_view(&mut self, kind: Kind, length: u32) -> ArrayBufferView {
        let byte_length = length as usize * kind.bytes_per_element();
        // Words, so the view is aligned for any element.
        let lane = fixed_lane::<u64>(byte_length.div_ceil(8));
        let at = lane.as_mut_ptr() as *mut u8;
        // SAFETY: the lane is at least `byte_length` bytes, and no one else's.
        unsafe { core::ptr::write_bytes(at, 0, byte_length) };
        #[cfg(target_arch = "wasm32")]
        let (buffer, byte_offset) = {
            // SAFETY: the module's memory, addressed from 0, never moves.
            let memory = MEMORY.get(|| unsafe { ArrayBuffer::from_raw(core::ptr::null_mut(), u32::MAX as usize) });
            (memory.clone(), at as usize)
        };
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the lane is leaked, so it outlives every view of it.
        let (buffer, byte_offset) = (unsafe { ArrayBuffer::from_raw(at, byte_length) }, 0);
        ArrayBufferView { buffer, byte_offset, byte_length, kind }
    }

    fn give(&mut self, _view: &ArrayBufferView) {}
}

/// A vector whose room is a lane of the fixed section: what a structure keeps
/// for the kernel's life, at a capacity decided up front. A push past it is
/// refused rather than grown into. The lane is taken at the first write, so a
/// structure made before the sections are laid out can hold one.
pub struct FixedVec<T: Copy + 'static> {
    items: Option<&'static mut [MaybeUninit<T>]>,
    capacity: usize,
    len: usize,
}

impl<T: Copy + 'static> FixedVec<T> {
    pub const fn new(capacity: usize) -> FixedVec<T> {
        FixedVec { items: None, capacity, len: 0 }
    }

    fn room(&mut self) -> &mut [MaybeUninit<T>] {
        let capacity = self.capacity;
        self.items.get_or_insert_with(|| fixed_lane(capacity))
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Appends `v`; false, and nothing appended, when the lane is full.
    pub fn push(&mut self, v: T) -> bool {
        if self.len == self.capacity {
            return false;
        }
        let len = self.len;
        self.room()[len].write(v);
        self.len += 1;
        true
    }

    /// Appends what `items` yields until the lane is full.
    pub fn extend(&mut self, items: impl IntoIterator<Item = T>) {
        for v in items {
            if !self.push(v) {
                return;
            }
        }
    }

    /// Puts `v` at `at`, moving what follows back by one; false when full.
    pub fn insert(&mut self, at: usize, v: T) -> bool {
        if self.len == self.capacity || at > self.len {
            return false;
        }
        let len = self.len;
        let room = self.room();
        room.copy_within(at..len, at + 1);
        room[at].write(v);
        self.len += 1;
        true
    }

    /// Takes the value at `at`, moving what follows forward by one.
    pub fn remove(&mut self, at: usize) -> T {
        let v = self[at];
        let len = self.len;
        self.room().copy_within(at + 1..len, at);
        self.len -= 1;
        v
    }

    /// Takes the value at `at`, the last moved into its place.
    pub fn swap_remove(&mut self, at: usize) -> T {
        let v = self[at];
        let last = self[self.len - 1];
        self[at] = last;
        self.len -= 1;
        v
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }

    /// Drops the first `n` values, moving the rest to the front.
    pub fn drain_front(&mut self, n: usize) {
        let n = n.min(self.len);
        if n == 0 {
            return;
        }
        let len = self.len;
        self.room().copy_within(n..len, 0);
        self.len -= n;
    }

    pub fn as_slice(&self) -> &[T] {
        match &self.items {
            // SAFETY: the first `len` are written.
            Some(items) => unsafe { core::slice::from_raw_parts(items.as_ptr() as *const T, self.len) },
            None => &[],
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match &mut self.items {
            // SAFETY: as `as_slice`.
            Some(items) => unsafe { core::slice::from_raw_parts_mut(items.as_mut_ptr() as *mut T, self.len) },
            None => &mut [],
        }
    }
}

impl<T: Copy + 'static> core::ops::Deref for FixedVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + 'static> core::ops::DerefMut for FixedVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<'a, T: Copy + 'static> IntoIterator for &'a FixedVec<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a InlineVec<T, N> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items[..self.len].iter()
    }
}

/// A short list kept inline, at a capacity decided up front: what a record
/// holds a few of, kept as plain data.
#[derive(Clone, Copy)]
pub struct InlineVec<T: Copy + Default, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> InlineVec<T, N> {
    pub fn new() -> InlineVec<T, N> {
        InlineVec { items: [T::default(); N], len: 0 }
    }

    /// Appends `v`; false, and nothing appended, when full.
    pub fn push(&mut self, v: T) -> bool {
        if self.len == N {
            return false;
        }
        self.items[self.len] = v;
        self.len += 1;
        true
    }

    /// Takes the value at `at`, moving what follows forward by one.
    pub fn remove(&mut self, at: usize) -> T {
        let v = self.items[at];
        self.items.copy_within(at + 1..self.len, at);
        self.len -= 1;
        v
    }

    /// Keeps the values `keep` says to, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let mut kept = 0;
        for i in 0..self.len {
            let v = self.items[i];
            if keep(&v) {
                self.items[kept] = v;
                kept += 1;
            }
        }
        self.len = kept;
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }

    /// Appends what `items` yields until the list is full.
    pub fn extend(&mut self, items: impl IntoIterator<Item = T>) {
        for v in items {
            if !self.push(v) {
                return;
            }
        }
    }

    /// Every value, the list left empty.
    pub fn take(&mut self) -> InlineVec<T, N> {
        core::mem::take(self)
    }
}

impl<T: Copy + Default, const N: usize> Default for InlineVec<T, N> {
    fn default() -> Self {
        InlineVec::new()
    }
}

impl<T: Copy + Default, const N: usize> core::ops::Deref for InlineVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items[..self.len]
    }
}

impl<T: Copy + Default, const N: usize> core::ops::DerefMut for InlineVec<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }
}

impl<T: Copy + Default, const N: usize> IntoIterator for InlineVec<T, N> {
    type Item = T;
    type IntoIter = core::iter::Take<core::array::IntoIter<T, N>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().take(self.len)
    }
}

/// What a LaneMap is keyed by: compared whole, placed by its hash.
pub trait LaneKey: Copy + Eq + 'static {
    fn hash(&self) -> u64;
}

/// splitmix64's finalizer: every input bit reaches every output bit.
const fn spread(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl LaneKey for u32 {
    fn hash(&self) -> u64 {
        spread(*self as u64)
    }
}

impl LaneKey for u64 {
    fn hash(&self) -> u64 {
        spread(*self)
    }
}

impl LaneKey for (i64, i64) {
    fn hash(&self) -> u64 {
        spread(self.0 as u64 ^ spread(self.1 as u64))
    }
}

impl LaneKey for (u64, u32, u32) {
    fn hash(&self) -> u64 {
        spread(self.0 ^ spread(((self.1 as u64) << 32) | self.2 as u64))
    }
}

/// A map whose room is two lanes of the fixed section, taken at the first
/// write: open addressing with linear probing over a power-of-two table kept
/// at most half full, and deletion by backward shift, so no probe stops early
/// and no tombstone piles up. A walk is the table's order, which the same
/// writes in the same order always leave the same.
pub struct LaneMap<K: LaneKey, V: Copy + 'static> {
    /// Slots in the table: twice the entries it holds, to a power of two.
    slots: usize,
    entries: Option<&'static mut [MaybeUninit<(K, V)>]>,
    used: Option<&'static mut [u8]>,
    len: usize,
}

/// A LaneMap's entries, written where `used` says.
type Entries<K, V> = [MaybeUninit<(K, V)>];

/// Closes the hole a removal left at `i`: an entry past it moves in unless its
/// home lies between the hole and it.
fn backshift<K: LaneKey, V: Copy>(entries: &mut Entries<K, V>, used: &mut [u8], mut i: usize) {
    let mask = used.len() - 1;
    loop {
        let mut j = i;
        loop {
            j = (j + 1) & mask;
            if used[j] == 0 {
                used[i] = 0;
                return;
            }
            // SAFETY: a used slot is written.
            let home = unsafe { entries[j].assume_init_ref() }.0.hash() as usize & mask;
            let stays = if i <= j { i < home && home <= j } else { i < home || home <= j };
            if !stays {
                entries[i] = entries[j];
                i = j;
                break;
            }
        }
    }
}

impl<K: LaneKey, V: Copy + 'static> LaneMap<K, V> {
    /// Room for `capacity` entries.
    pub const fn new(capacity: usize) -> LaneMap<K, V> {
        LaneMap {
            slots: capacity.next_power_of_two() * 2,
            entries: None,
            used: None,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn lanes(&mut self) -> (&mut Entries<K, V>, &mut [u8]) {
        if self.used.is_none() {
            let used = fixed_lane::<u8>(self.slots);
            for b in used.iter_mut() {
                b.write(0);
            }
            // SAFETY: every byte was just written.
            self.used = Some(unsafe { &mut *(used as *mut [MaybeUninit<u8>] as *mut [u8]) });
            self.entries = Some(fixed_lane(self.slots));
        }
        (self.entries.as_deref_mut().expect("taken with used"), self.used.as_deref_mut().expect("just taken"))
    }

    fn find(&self, key: &K) -> Option<usize> {
        let (entries, used) = (self.entries.as_deref()?, self.used.as_deref()?);
        let mask = self.slots - 1;
        let mut i = key.hash() as usize & mask;
        loop {
            if used[i] == 0 {
                return None;
            }
            // SAFETY: a used slot is written.
            if unsafe { entries[i].assume_init_ref() }.0 == *key {
                return Some(i);
            }
            i = (i + 1) & mask;
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let i = self.find(key)?;
        // SAFETY: a found slot is written.
        Some(unsafe { &self.entries.as_deref()?[i].assume_init_ref().1 })
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let i = self.find(key)?;
        // SAFETY: a found slot is written.
        Some(unsafe { &mut self.entries.as_deref_mut()?[i].assume_init_mut().1 })
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    /// Sets `key`'s value; the value it had.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(i) = self.find(&key) {
            // SAFETY: a found slot is written.
            let entry = unsafe { self.entries.as_deref_mut()?[i].assume_init_mut() };
            return Some(core::mem::replace(&mut entry.1, value));
        }
        assert!(self.len < self.slots / 2, "a map past the {} entries it was sized for", self.slots / 2);
        let mask = self.slots - 1;
        let (entries, used) = self.lanes();
        let mut i = key.hash() as usize & mask;
        while used[i] != 0 {
            i = (i + 1) & mask;
        }
        used[i] = 1;
        entries[i].write((key, value));
        self.len += 1;
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let i = self.find(key)?;
        let (entries, used) = self.lanes();
        // SAFETY: a found slot is written.
        let value = unsafe { entries[i].assume_init_ref() }.1;
        backshift(entries, used, i);
        self.len -= 1;
        Some(value)
    }

    /// Every entry, in the table's order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let entries = self.entries.as_deref().unwrap_or(&[]);
        let used = self.used.as_deref().unwrap_or(&[]);
        used.iter().zip(entries).filter(|(u, _)| **u != 0).map(|(_, e)| {
            // SAFETY: a used slot is written.
            let e = unsafe { e.assume_init_ref() };
            (&e.0, &e.1)
        })
    }

    pub fn clear(&mut self) {
        if let Some(used) = self.used.as_deref_mut() {
            used.fill(0);
        }
        self.len = 0;
    }
}

impl<K: LaneKey, V: Copy + 'static> core::ops::Index<&K> for LaneMap<K, V> {
    type Output = V;

    /// `key`'s value, which the caller knows is there.
    fn index(&self, key: &K) -> &V {
        self.get(key).expect("a key the map holds")
    }
}

/// Where a string lies in a TextLane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    at: u32,
    len: u32,
}

/// Strings kept one after another in a lane of the fixed section, each named
/// by its span: a table whose records hold text holds spans, and its text is
/// cleared with it.
pub struct TextLane {
    bytes: FixedVec<u8>,
}

struct LaneWrite<'a>(&'a mut FixedVec<u8>);

impl fmt::Write for LaneWrite<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.0.capacity() - self.0.len() < s.len() {
            return Err(fmt::Error);
        }
        for &b in s.as_bytes() {
            self.0.push(b);
        }
        Ok(())
    }
}

impl TextLane {
    pub fn new(capacity: usize) -> TextLane {
        TextLane { bytes: FixedVec::new(capacity) }
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
    }

    /// What `v` displays as, appended: its span, or none — and nothing of it
    /// kept — when it does not fit.
    pub fn push(&mut self, v: impl fmt::Display) -> Option<Span> {
        let at = self.bytes.len();
        if write!(LaneWrite(&mut self.bytes), "{v}").is_err() {
            self.bytes.truncate(at);
            return None;
        }
        Some(Span { at: at as u32, len: (self.bytes.len() - at) as u32 })
    }

    pub fn get(&self, span: Span) -> &str {
        let (at, len) = (span.at as usize, span.len as usize);
        // SAFETY: only whole `str`s are appended, and a span names one of them
        // or none (the default names nothing).
        unsafe { core::str::from_utf8_unchecked(&self.bytes.as_slice()[at..at + len]) }
    }
}

#[cfg(test)]
#[path = "memory.test.rs"]
mod test;
