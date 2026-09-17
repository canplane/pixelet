// pxl_core/layer/decor/instance.rs

//! A chunk's decor instances as lanes, and the block the scheduler hands them
//! over in.
//!
//! What an instance is: a chunk-local position, a spin about Z, a scale, and
//! where it came from — a user-placed entry in object/store.rs, or a procedural
//! scatter that no store names. The id is not stored: a placed entry is named
//! by its band and packed position in the store's own lanes, two u32s, and the
//! string id is rebuilt from those only when a hit needs it (object/store.rs's
//! entry_id_of). `hidden` is a flag lane rather than a separate list: the vertex
//! shader sends a hidden instance outside the clip volume (decor.wgsl) while the
//! hit test keeps seeing it.
//!
//! The scheduler builds a chunk's decor at one tier as one block of the mesh
//! section (crates/pxl/core/src/layer/decor/build.rs's DecorBuild): every group's
//! lanes — a group per (kind, variant) in the order the kinds were taught —
//! laid out here.

/// NO_ENTRY: a procedural instance resolves to no stored entry.
///
/// `i32::MIN` and not `u32::MAX`, because the word it fills holds a `BandZ` —
/// an `i32` — cast to `u32`, and the band below the origin is -1, which casts
/// to exactly `u32::MAX`. A decor placed there read as procedural: the raycast
/// passes over NO_ENTRY (world/chunk/raycast.rs), so the hit
/// never resolved to its entry and the thing could not be picked or erased. Any value a band can
/// take has to stay out of the sentinel, and a band is `z >> BAND_SHAMT` over
/// a sixteen-bit Z lane (spec/space/pvf3's IVEC_Z_W), so it fits in ten bits
/// signed and nothing reaches `i32::MIN`.
use xpute_core::sys::mem::{alloc, Guard};

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

/// Instances one chunk's decor holds at most at a tier: a kind scatters at
/// most one to a cell, and the placed entries come past them. What does not
/// fit is left out.
const DECOR_INSTANCES: usize = 1 << 16;

/// Where a chunk's instances are gathered, in the order they are made, before
/// they are laid into its block group by group: two lanes in blocks of the mesh
/// section, taken once and kept by the scheduler, so a build takes nothing of
/// its own and the megabytes they are sit outside the module heap.
pub struct DecorScratch {
    pub(crate) instances: &'static mut [Instance],
    /// The group each instance is in.
    pub(crate) groups: &'static mut [u16],
    pub(crate) len: usize,
    pub(crate) counts: [u32; DECOR_GROUPS],
    pub(crate) group_count: usize,
    /// Instances the lanes had no room for, since the kernel started: what a
    /// chunk shows is short by this many, and the ledger says so rather than
    /// leaving the gap to be noticed on screen.
    pub dropped: u32,
}

impl DecorScratch {
    /// Its lanes, taken from the heap; none when it has no room for them.
    pub fn new() -> Option<DecorScratch> {
        let instance_bytes = (DECOR_INSTANCES * core::mem::size_of::<Instance>()) as u32;
        let instances = Guard::alloc(instance_bytes)?;
        let (groups, _) = alloc((DECOR_INSTANCES * core::mem::size_of::<u16>()) as u32)?;
        let instances = instances.keep();
        // SAFETY: two blocks of the pool this scratch holds alone and never
        // gives back, each DECOR_INSTANCES of its lane's element and aligned
        // past it; the elements are plain numbers, and none is read before a
        // build has written it.
        let (instances, groups) = unsafe {
            (
                core::slice::from_raw_parts_mut(instances.0 as *mut Instance, DECOR_INSTANCES),
                core::slice::from_raw_parts_mut(groups as *mut u16, DECOR_INSTANCES),
            )
        };
        Some(DecorScratch {
            instances,
            groups,
            len: 0,
            counts: [0; DECOR_GROUPS],
            group_count: 0,
            dropped: 0,
        })
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

/// Each group's first lane word in a block and its count, in group order, as
/// the block's own header says: what a draw of the group reads its instances
/// by.
pub fn decor_groups(words: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let groups = words.first().map_or(0, |&g| g as usize);
    let mut at = 1 + groups;
    words.get(1..1 + groups).unwrap_or(&[]).iter().map(move |&n| {
        let (first, n) = (at, n as usize);
        at += 7 * n + n.div_ceil(4);
        (first as u32, n as u32)
    })
}
