// pxl-kernel/layer/decor/instance.rs

//! A decor cell's instances as lanes, and the block they are laid into.
//!
//! What an instance is: a cell-local position, a spin about Z, a scale, and
//! where it came from — a user-placed entry in object/store.rs, or a procedural
//! scatter that no store names. A placed entry is named by its band and packed
//! position in its column, two u32s, which with its column is the
//! store's own address for it (object/store.rs `EntryAt`). `hidden` is a flag
//! lane rather than a separate list: the presentation leaves a hidden
//! instance out of what is drawn (layer/decor/cell.rs `present`) while the
//! hit test keeps seeing it.
//!
//! A decor cell's population is one block of the heap (layer/decor/build.rs
//! `DecorBuild`): every group's lanes — a group per (kind, variant) in the
//! order the kinds were taught — laid out here.

/// NO_ENTRY: a procedural instance resolves to no stored entry.
///
/// `i32::MIN` and not `u32::MAX`, because the word it fills holds a `BandZ` —
/// an `i32` — cast to `u32`, and the band below the origin is -1, which casts
/// to exactly `u32::MAX`. A decor placed there read as procedural: the hit
/// passes over NO_ENTRY (layer/decor/hit.rs), so it
/// never resolved to its entry and the thing could not be picked or erased. Any value a band can
/// take has to stay out of the sentinel, and a band is a cell's Z divided by
/// BAND_CELLS, from a sixteen-bit Z lane (spec/space/pvf3's IVEC_Z_W), so it
/// fits in ten bits signed and nothing reaches `i32::MIN`.
use std::collections::TryReserveError;

pub const NO_ENTRY: u32 = i32::MIN as u32;
pub const HIDDEN_FLAG: u8 = 1;

/// One instance, as the lanes hold it.
#[derive(Clone, Copy)]
pub(crate) struct Instance {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    pub(crate) rotation: f32,
    pub(crate) scale: f32,
    pub(crate) flags: u8,
    pub(crate) band: u32,
    pub(crate) pos: u32,
}

/// Groups one block holds at most: as many as the renderer's decor pass draws.
pub const DECOR_GROUPS: usize = 64;

/// Where a cell's instances are gathered, in the order they are made, before
/// they are laid into its block group by group: two lanes of the heap, one
/// cell's at a time, kept from cell to cell so a settled turn asks the heap
/// for nothing.
///
/// Not a range of `.fixed`: a cell holds a scattered instance a cell of its
/// lattice a kind at most, but the placed ones are as many as were placed,
/// and no size holds every cell. A workspace of a fixed size would leave
/// out what did not fit, and what was left out could be what someone put
/// there. The lanes grow to the largest cell gathered, and nothing is left
/// out: where the heap refuses them more, the cell is not built
/// (layer/decor/cell.rs `build`).
pub struct DecorScratch {
    pub(crate) instances: Vec<Instance>,
    /// The group each instance is in.
    pub(crate) groups: Vec<u16>,
    pub(crate) counts: [u32; DECOR_GROUPS],
    pub(crate) group_count: usize,
}

/// The fewest instances the lanes grow by: a cell of a single kind's scatter
/// is some thousands, and growing a few at a time would ask the heap once a
/// handful of instances.
const MIN_GROWTH: usize = 1024;

impl Default for DecorScratch {
    fn default() -> DecorScratch {
        DecorScratch {
            instances: Vec::new(),
            groups: Vec::new(),
            counts: [0; DECOR_GROUPS],
            group_count: 0,
        }
    }
}

impl DecorScratch {
    pub(crate) fn clear(&mut self) {
        self.instances.clear();
        self.groups.clear();
        self.counts = [0; DECOR_GROUPS];
        self.group_count = 0;
    }

    /// One more instance, in `group`; an error, and nothing pushed, where
    /// the lanes are full and the heap refuses them more. They grow by
    /// doubling, asked for only when full, so a build that fits what an
    /// earlier one grew them to asks for nothing.
    pub(crate) fn push(&mut self, group: usize, instance: Instance) -> Result<(), TryReserveError> {
        if self.instances.len() == self.instances.capacity() || self.groups.len() == self.groups.capacity() {
            let more = self.instances.len().max(MIN_GROWTH);
            self.instances.try_reserve(more)?;
            self.groups.try_reserve(more)?;
        }
        self.instances.push(instance);
        self.groups.push(group as u16);
        self.counts[group] += 1;
        Ok(())
    }
}

/// A block is a word of group count, a word of instance count a group, then
/// each group's lanes. A group of `count` instances is five lanes of f32 —
/// x, y, z, the spin and the scale — the flags a byte each padded to a word,
/// then the band and the packed position: where each lies, in words from the
/// block's start, is this and nothing else's.
#[derive(Clone, Copy)]
pub struct GroupLanes {
    /// The group's first word.
    pub first: usize,
    pub count: usize,
}

impl GroupLanes {
    /// The words a group of `count` instances takes.
    pub const fn words(count: usize) -> usize {
        7 * count + count.div_ceil(4)
    }

    /// The word of instance `i` in f32 lane `lane`: 0 x, 1 y, 2 z, 3 the
    /// spin, 4 the scale.
    pub const fn f32_at(self, lane: usize, i: usize) -> usize {
        self.first + lane * self.count + i
    }

    /// The first word of the flags, a byte an instance.
    pub const fn flags(self) -> usize {
        self.first + 5 * self.count
    }

    /// The word of instance `i`'s band, and of its packed position.
    pub const fn band_at(self, i: usize) -> usize {
        self.flags() + self.count.div_ceil(4) + i
    }

    pub const fn pos_at(self, i: usize) -> usize {
        self.band_at(i) + self.count
    }
}

/// Each group's first lane word in a block and its count, in group order, as
/// the block's own header says: what a draw of the group reads its instances
/// by.
pub fn decor_groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words.get(1..1 + groups).unwrap_or(&[]).iter().map(move |&n| {
        let (first, n) = (at, n as usize);
        at += GroupLanes::words(n);
        (first as u32, n as u32)
    })
}
