// pxl-kernel/layer/decor/instance.rs

//! A chunk's decor instances as lanes, and the block the scheduler hands them
//! over in.
//!
//! What an instance is: a chunk-local position, a spin about Z, a scale, and
//! where it came from — a user-placed entry in object/store.rs, or a procedural
//! scatter that no store names. A placed entry is named by its band and packed
//! position in its column, two u32s, which with the chunk's column is the
//! store's own address for it (object/store.rs `EntryAt`). `hidden` is a flag
//! lane rather than a separate list: the vertex
//! shader sends a hidden instance outside the clip volume (decor.wgsl) while the
//! hit test keeps seeing it.
//!
//! The scheduler builds a chunk's decor at one tier as one block of the mesh
//! section (crates/pxl/web/kernel/src/layer/decor/build.rs's DecorBuild): every group's
//! lanes — a group per (kind, variant) in the order the kinds were taught —
//! laid out here.

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
use crate::mem::fixed::{lane, range, Range};

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

use crate::param::DECOR_INSTANCES;

/// Bytes of the instances' lane in `.fixed.decor`, which the groups' follows.
const INSTANCE_LANE_BYTES: u32 = (DECOR_INSTANCES * core::mem::size_of::<Instance>()) as u32;

/// `.fixed.decor`: the instances' lane, then the groups'.
pub const DECOR_BYTES: u32 = INSTANCE_LANE_BYTES + (DECOR_INSTANCES * core::mem::size_of::<u16>()) as u32;

/// Where a chunk's instances are gathered, in the order they are made, before
/// they are laid into its block group by group: two lanes in `.fixed.decor`,
/// one chunk's at a time, so a build takes nothing of its own.
pub struct DecorScratch {
    pub(crate) instances: &'static mut [Instance],
    /// The group each instance is in.
    pub(crate) groups: &'static mut [u16],
    pub(crate) len: usize,
    pub(crate) counts: [u32; DECOR_GROUPS],
    pub(crate) group_count: usize,
    /// Instances the lanes had no room for, since the kernel started: what a
    /// chunk shows is short by this many, and the profiler says so rather than
    /// leaving the gap to be noticed on screen.
    pub dropped: u32,
}

impl DecorScratch {
    /// Its lanes in `.fixed.decor`, as the module laid them. Made once: the
    /// scheduler keeps the one there is.
    pub fn laid() -> DecorScratch {
        let decor = range(Range::Decor);
        // SAFETY: the range's two lanes, which this scratch holds alone; the
        // elements are plain numbers, and none is read before a build has
        // written it.
        let (instances, groups) = unsafe { (lane(decor, 0, DECOR_INSTANCES), lane(decor, INSTANCE_LANE_BYTES, DECOR_INSTANCES)) };
        DecorScratch {
            instances,
            groups,
            len: 0,
            counts: [0; DECOR_GROUPS],
            group_count: 0,
            dropped: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.counts = [0; DECOR_GROUPS];
        self.group_count = 0;
    }

    pub(crate) fn push(&mut self, group: usize, instance: Instance) {
        if self.len == DECOR_INSTANCES {
            self.dropped += 1;
            return;
        }
        self.instances[self.len] = instance;
        self.groups[self.len] = group as u16;
        self.len += 1;
        self.counts[group] += 1;
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
