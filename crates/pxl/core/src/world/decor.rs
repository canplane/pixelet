//! A chunk's decor at a tier: module/world/decor/{scatter,decor_layer}.ts's
//! build, in Rust.
//!
//! Two halves. The procedural one scatters each kind over the chunk's cells
//! with a random stream seeded from the page, the kind and the cell alone —
//! the same stream the TypeScript drew, bit for bit, so a tree stands where
//! it stood — keeps what lands on eligible ground at the kind's density, and
//! stands it on the terrain unless a placed object occupies that cell. The
//! user one is every decor entry the object store holds in the column,
//! turned and varied by a stream seeded from its own corner. What comes out
//! is one block: for each kind and variant, the instances' lanes.
//!
//! A kind is drawn on a tier only where it is at least two of the tier's
//! cells across; why, and every other rule, is the TypeScript's.

use crate::bake::terrain::{ChunkSample, PageView};
use crate::space::constant::{CHUNK_SPAN, PAGE_CELL_SPAN, U_PER_CELL};
use crate::world::object::{unpack_pos, Store, BAND_CELLS, CELL_STEPS, STEP_UNITS};
use pxl_shared::spec::space::pvf3::ivec::{gather, IVec};
use xpute_core::kit::rng::{derive_seed_u64 as derive_seed, u32_to_unit, SplitMix64};

/// decor_kinds.ts's DECOR_MASTER_SEED is written with 80 bits; every use
/// takes it modulo 2^64 (kit/rng.ts's derive_seed_u64 masks the XOR), so
/// these are the bits that ever mattered.
const MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;
const SCALE_JITTER: f64 = 0.18;
const ALIGN_NEIGHBOR_RADIUS: u32 = 3;
/// NO_ENTRY: a procedural instance resolves to no stored entry.
pub const NO_ENTRY: u32 = 0xffff_ffff;
pub const HIDDEN_FLAG: u8 = 1;

/// decor_build.ts's stream: kit/rng.ts's make_rng, drawn as units.
struct Rng(SplitMix64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(SplitMix64::new(seed))
    }

    fn unit(&mut self) -> f64 {
        u32_to_unit(self.0.next_u32())
    }
}

/// What a channel's type must be for a kind to stand on the cell.
#[derive(Clone, Copy)]
pub enum Rule {
    Any,
    Void,
    /// One of the types whose bits are set.
    OneOf([u64; 4]),
}

impl Rule {
    fn allows(&self, t: u8) -> bool {
        match self {
            Rule::Any => true,
            Rule::Void => t == 0,
            Rule::OneOf(bits) => bits[(t >> 6) as usize] & (1 << (t & 63)) != 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Kind {
    pub tag: u64,
    pub density: f64,
    pub variant_count: u32,
    pub align_transport: bool,
    /// The largest dimension of the kind's models, in world units.
    pub extent: f64,
    /// The object store's value a placed instance of this kind carries.
    pub decor_value: Option<u32>,
    /// Land, water, structure, transport.
    pub rules: [Rule; 4],
}

/// One instance, as the lanes hold it.
#[derive(Clone, Copy)]
struct Instance {
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    scale: f32,
    flags: u8,
    band: u32,
    pos: u32,
}

/// module/world/lod.ts's TARGET_CELL_PX.
fn target_cell_px() -> f64 {
    let px_per_unit = 1080.0 / (2.0 * (50.0 * core::f64::consts::PI / 360.0).tan());
    px_per_unit / 192.0
}

/// decor_kinds.ts's decor_density: 1, or 0 where the kind is under two of
/// the tier's cells across.
pub fn density(extent: f64, step: u32) -> f64 {
    let t = target_cell_px();
    let projected = (extent / U_PER_CELL) * t / (step as f64);
    if projected < t * 2.0 {
        0.0
    } else {
        1.0
    }
}

fn transport_aligned_rotation(transport: &[u8], cell_x: u32, cell_y: u32, flip: bool) -> f64 {
    let idx = (cell_y * PAGE_CELL_SPAN + cell_x) as usize;
    let span = PAGE_CELL_SPAN as usize;
    let (mut h, mut v) = (0, 0);
    for d in 1..=ALIGN_NEIGHBOR_RADIUS {
        let du = d as usize;
        if cell_x >= d && transport[idx - du] != 0 {
            h += 1;
        }
        if cell_x + d < PAGE_CELL_SPAN && transport[idx + du] != 0 {
            h += 1;
        }
        if cell_y >= d && transport[idx - du * span] != 0 {
            v += 1;
        }
        if cell_y + d < PAGE_CELL_SPAN && transport[idx + du * span] != 0 {
            v += 1;
        }
    }
    let axis = if h > v { core::f64::consts::PI / 2.0 } else { 0.0 };
    if flip {
        axis + core::f64::consts::PI
    } else {
        axis
    }
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
    instances: &'static mut [Instance],
    /// The group each instance is in.
    groups: &'static mut [u16],
    len: usize,
    counts: [u32; DECOR_GROUPS],
    group_count: usize,
    /// Instances the lanes had no room for, since the kernel started: what a
    /// chunk shows is short by this many, and the ledger says so rather than
    /// leaving the gap to be noticed on screen.
    pub dropped: u32,
}

impl DecorScratch {
    /// Its lanes, taken from the heap; none when it has no room for them.
    pub fn new() -> Option<DecorScratch> {
        let instance_bytes = (DECOR_INSTANCES * core::mem::size_of::<Instance>()) as u32;
        let instances = crate::bake::volume::take_bytes(instance_bytes as usize)?;
        let Some(groups) = crate::bake::volume::take_bytes(DECOR_INSTANCES * core::mem::size_of::<u16>()) else {
            // SAFETY: just taken, above.
            unsafe { crate::bake::volume::give_bytes(instances, instance_bytes as usize) };
            return None;
        };
        // SAFETY: two blocks of the pool this scratch holds alone and never
        // gives back, each DECOR_INSTANCES of its lane's element and aligned
        // past it; the elements are plain numbers, and none is read before a
        // build has written it.
        let (instances, groups) = unsafe {
            (
                core::slice::from_raw_parts_mut(instances as *mut Instance, DECOR_INSTANCES),
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

    fn clear(&mut self) {
        self.len = 0;
        self.counts = [0; DECOR_GROUPS];
        self.group_count = 0;
    }

    fn push(&mut self, group: usize, instance: Instance) {
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

/// A chunk's decor at a tier, gathered and ready to be laid into its block.
pub struct DecorBuild<'s> {
    scratch: &'s DecorScratch,
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

/// What a chunk's decor is built from.
pub struct Chunk<'a> {
    pub page: &'a PageView<'a>,
    pub page_idx: IVec,
    /// The column this chunk stands in, and its X and Y lanes.
    pub col_idx: IVec,
    pub col_x: i64,
    pub col_y: i64,
    pub tx0: u32,
    pub ty0: u32,
    pub show_decor: bool,
    /// The hidden entry's band and packed position in this column.
    pub hide: Option<(i32, u32)>,
}

impl<'s> DecorBuild<'s> {
    /// Builds every kind's instances for a chunk at `step` into `scratch`,
    /// the sample already read for it (its elevations).
    pub fn build(scratch: &'s mut DecorScratch, kinds: &[Kind], chunk: &Chunk, sample: &ChunkSample, store: &Store, step: u32) -> DecorBuild<'s> {
        scratch.clear();
        let page_seed = derive_seed(MASTER_SEED, chunk.page_idx);
        let lanes = store.chunk_lanes(chunk.col_idx);
        // A chunk with no entry has no cell to look up: most chunks.
        let has_entries = lanes.as_ref().is_some_and(|l| l.count > 0);
        for kind in kinds {
            // A kind with no variant has no group to fill and no variant to
            // pick below; the groups are the kinds' variants in order.
            if kind.variant_count == 0 {
                continue;
            }
            // The kind's variants take the next groups; past what a block
            // holds, the kinds after have none.
            let first = scratch.group_count;
            if first + kind.variant_count as usize > DECOR_GROUPS {
                break;
            }
            scratch.group_count += kind.variant_count as usize;

            // Placed instances first, in the store's entry order.
            if let (Some(value), Some(l)) = (kind.decor_value, &lanes) {
                // SAFETY: the column's lanes, read before anything writes the store.
                let (bands, poss, states) = unsafe {
                    let n = l.count as usize;
                    (
                        core::slice::from_raw_parts(l.band_at, n),
                        core::slice::from_raw_parts(l.pos_at, n),
                        core::slice::from_raw_parts(l.state_at, n),
                    )
                };
                for k in 0..bands.len() {
                    if states[k] != value {
                        continue;
                    }
                    let pos = poss[k];
                    let (x, y, z, level) = unpack_pos(pos);
                    let size = 2f64.powi(level);
                    let corner = gather(
                        chunk.col_x + (x / CELL_STEPS) as i64,
                        chunk.col_y + (y / CELL_STEPS) as i64,
                        bands[k] as i64 * BAND_CELLS as i64 + (z / CELL_STEPS) as i64,
                    );
                    let mut rng = Rng::new(derive_seed(MASTER_SEED, corner));
                    let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
                    let rotation = rng.unit() * core::f64::consts::PI * 2.0;
                    let hidden = chunk.hide == Some((bands[k], pos));
                    let h = size / 2.0;
                    scratch.push(first + variant as usize, Instance {
                        x: (x as f64 * STEP_UNITS + h) as f32,
                        y: (y as f64 * STEP_UNITS + h) as f32,
                        z: (bands[k] as f64 * BAND_CELLS as f64 + z as f64 * STEP_UNITS) as f32,
                        rotation: rotation as f32,
                        scale: 1.0,
                        flags: if hidden { HIDDEN_FLAG } else { 0 },
                        band: bands[k] as u32,
                        pos,
                    });
                }
            }

            // Then the scatter, where the kind is drawn on this tier.
            let d = density(kind.extent, step);
            if chunk.show_decor && d > 0.0 {
                let kind_seed = derive_seed(page_seed, kind.tag);
                let g = &chunk.page.ground;
                for dy in 0..CHUNK_SPAN {
                    let cell_y = chunk.ty0 + dy;
                    for dx in 0..CHUNK_SPAN {
                        let cell_x = chunk.tx0 + dx;
                        let idx = (cell_y * PAGE_CELL_SPAN + cell_x) as usize;
                        if !(0..4).all(|c| kind.rules[c].allows(g[c][idx])) {
                            continue;
                        }
                        let mut rng = Rng::new(derive_seed(kind_seed, ((cell_x as u64) << 16) | cell_y as u64));
                        if rng.unit() >= kind.density * d {
                            continue;
                        }
                        let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
                        let rotation = if kind.align_transport {
                            let flip = rng.unit() < 0.5;
                            transport_aligned_rotation(g[3], cell_x, cell_y, flip)
                        } else {
                            rng.unit() * core::f64::consts::PI * 2.0
                        };
                        let scale = 1.0 + (rng.unit() * 2.0 - 1.0) * SCALE_JITTER;
                        let ele = sample.ele[(dy * CHUNK_SPAN + dx) as usize] as i32;
                        if has_entries && store.cell(chunk.col_idx, dx, dy, ele).is_some_and(|c| c.solid > 0) {
                            continue;
                        }
                        scratch.push(first + variant as usize, Instance {
                            x: (dx as f64 + 0.5) as f32,
                            y: (dy as f64 + 0.5) as f32,
                            z: ele as f32,
                            rotation: rotation as f32,
                            scale: scale as f32,
                            flags: 0,
                            band: NO_ENTRY,
                            pos: 0,
                        });
                    }
                }
            }
        }
        DecorBuild { scratch }
    }

    /// The block's size in bytes: a word of group count, a word of count a
    /// group, then each group's lanes — five of f32, the flags padded to a
    /// word, the band and the position.
    pub fn bytes(&self) -> u32 {
        let s = self.scratch;
        let words: usize = 1 + s.group_count + s.counts[..s.group_count].iter().map(|&n| 7 * n as usize + (n as usize).div_ceil(4)).sum::<usize>();
        (words * 4) as u32
    }

    /// Writes the block: each group's lanes at the group's place, each
    /// instance at its rank among its group's, in the order they were made.
    ///
    /// # Safety
    /// `at` must address `bytes()` bytes, 4-aligned, the caller's to write.
    pub unsafe fn write(&self, at: *mut u8) {
        let s = self.scratch;
        let words = at as *mut u32;
        let groups = s.group_count;
        *words = groups as u32;
        let mut first = [0usize; DECOR_GROUPS];
        let mut k = 1 + groups;
        for (g, start) in first.iter_mut().enumerate().take(groups) {
            let n = s.counts[g] as usize;
            *words.add(1 + g) = n as u32;
            *start = k;
            // The flags are bytes padded to a word, and a pad byte is zero.
            core::ptr::write_bytes(at.add((k + 5 * n) * 4), 0, n.div_ceil(4) * 4);
            k += 7 * n + n.div_ceil(4);
        }
        let mut rank = [0usize; DECOR_GROUPS];
        for (i, &g) in s.instances[..s.len].iter().zip(&s.groups[..s.len]) {
            let g = g as usize;
            let (n, r, f) = (s.counts[g] as usize, rank[g], first[g]);
            for (lane, v) in [i.x, i.y, i.z, i.rotation, i.scale].into_iter().enumerate() {
                *words.add(f + lane * n + r) = v.to_bits();
            }
            *at.add((f + 5 * n) * 4 + r) = i.flags;
            let past_flags = f + 5 * n + n.div_ceil(4);
            *words.add(past_flags + r) = i.band;
            *words.add(past_flags + n + r) = i.pos;
            rank[g] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_draw_rounds_the_word_to_a_double_first_as_javascript_does() {
        // A word past 2^53 whose low bits differ from its double's.
        let z: u64 = 0xffff_ffff_ffff_fdff;
        assert_eq!(((z as f64) % 4_294_967_296.0) as u32, 0, "rounds up to 2^64, which is 0 mod 2^32");
        assert_ne!(z as u32, 0);
    }
}
