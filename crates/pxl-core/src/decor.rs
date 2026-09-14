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

use crate::object::{Store, POS_BITS};
use crate::terrain::{ChunkSample, PageView, CHUNK_SPAN, PAGE_CELL_SPAN};

/// decor_layer.ts's DECOR_MASTER_SEED is written with 80 bits; every use
/// takes it modulo 2^64 (kit/rng.ts's derive_seed_u64 masks the XOR), so
/// these are the bits that ever mattered.
const MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;
const SEED_SALT: u64 = 0x9e37_79b9_7f4a_7c15;
const SCALE_JITTER: f64 = 0.18;
const ALIGN_NEIGHBOR_RADIUS: u32 = 3;
/// NO_ENTRY: a procedural instance resolves to no stored entry.
pub const NO_ENTRY: u32 = 0xffff_ffff;
pub const HIDDEN_FLAG: u8 = 1;
/// bake/constant.ts's U_PER_CELL.
const U_PER_CELL: f64 = 1.0;

fn mix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(SEED_SALT);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn derive_seed(base: u64, tag: u64) -> u64 {
    mix64(base ^ tag)
}

/// kit/rng.ts's make_rng: splitmix64, and a draw is `U32(Number(z))` — the
/// 64-bit word rounded to the nearest double first, then taken modulo 2^32,
/// which is not the word's low bits once it is past 2^53.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(SEED_SALT);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        ((z as f64) % 4_294_967_296.0) as u32
    }

    fn unit(&mut self) -> f64 {
        self.next() as f64 / 4_294_967_296.0
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

/// render/lod.ts's TARGET_CELL_PX.
fn target_cell_px() -> f64 {
    let px_per_unit = 1080.0 / (2.0 * (50.0 * core::f64::consts::PI / 360.0).tan());
    px_per_unit / 192.0
}

/// decor_layer.ts's decor_density: 1, or 0 where the kind is under two of
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

/// The lanes one block holds, and where each group starts.
pub struct DecorBuild {
    groups: Vec<Vec<Instance>>,
}

/// What a chunk's decor is built from.
pub struct Chunk<'a> {
    pub page: &'a PageView<'a>,
    pub page_idx: u64,
    /// The column's base: the object store's key, and its X and Y lanes.
    pub key: u64,
    pub key_x: i64,
    pub key_y: i64,
    pub tx0: u32,
    pub ty0: u32,
    pub show_decor: bool,
    /// The hidden entry's band and packed position in this column.
    pub hide: Option<(i32, u32)>,
}

/// pvf3/ivec.ts's gather.
fn gather(x: i64, y: i64, z: i64) -> u64 {
    ((x as u64 & 0xff_ffff) << 40) | ((y as u64 & 0xff_ffff) << 16) | (z as u64 & 0xffff)
}

impl DecorBuild {
    /// Builds every kind's instances for a chunk at `step`, the sample
    /// already read for it (its elevations).
    pub fn build(kinds: &[Kind], chunk: &Chunk, sample: &ChunkSample, store: &Store, step: u32) -> DecorBuild {
        let mut groups: Vec<Vec<Instance>> = Vec::new();
        let page_seed = derive_seed(MASTER_SEED, chunk.page_idx);
        let lanes = store.chunk_lanes(chunk.key);
        for kind in kinds {
            let mut by_variant: Vec<Vec<Instance>> = vec![Vec::new(); kind.variant_count as usize];

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
                    let (x, y, z) = (pos & 511, (pos >> POS_BITS) & 511, (pos >> (2 * POS_BITS)) & 511);
                    let level = (pos >> (3 * POS_BITS)) as i32 - 3;
                    let size = 2f64.powi(level);
                    let corner = gather(chunk.key_x + (x / 8) as i64, chunk.key_y + (y / 8) as i64, bands[k] as i64 * 64 + (z / 8) as i64);
                    let mut rng = Rng(derive_seed(MASTER_SEED, corner));
                    let variant = ((rng.unit() * kind.variant_count as f64).floor() as u32).min(kind.variant_count - 1);
                    let rotation = rng.unit() * core::f64::consts::PI * 2.0;
                    let hidden = chunk.hide == Some((bands[k], pos));
                    let h = size / 2.0;
                    by_variant[variant as usize].push(Instance {
                        x: (x as f64 * 0.125 + h) as f32,
                        y: (y as f64 * 0.125 + h) as f32,
                        z: (bands[k] as f64 * 64.0 + z as f64 * 0.125) as f32,
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
                        let mut rng = Rng(derive_seed(kind_seed, ((cell_x as u64) << 16) | cell_y as u64));
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
                        if store.cell(chunk.key, dx, dy, ele).is_some_and(|c| c.solid > 0) {
                            continue;
                        }
                        by_variant[variant as usize].push(Instance {
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
            groups.extend(by_variant);
        }
        DecorBuild { groups }
    }

    /// The block's size in bytes: a word of group count, a word of count a
    /// group, then each group's lanes — five of f32, the flags padded to a
    /// word, the band and the position.
    pub fn bytes(&self) -> u32 {
        let mut words = 1 + self.groups.len();
        for g in &self.groups {
            words += 7 * g.len() + g.len().div_ceil(4);
        }
        (words * 4) as u32
    }

    /// Each group's first lane word in the block and its count, in group
    /// order: what a draw of the group reads its instances by.
    pub fn group_layout(&self) -> Vec<(u32, u32)> {
        let mut out = Vec::with_capacity(self.groups.len());
        let mut k = 1 + self.groups.len();
        for g in &self.groups {
            out.push((k as u32, g.len() as u32));
            k += 7 * g.len() + g.len().div_ceil(4);
        }
        out
    }

    /// Writes the block.
    ///
    /// # Safety
    /// `at` must address `bytes()` bytes, 4-aligned, the caller's to write.
    pub unsafe fn write(&self, at: *mut u8) {
        let words = core::slice::from_raw_parts_mut(at as *mut u32, self.bytes() as usize / 4);
        let mut k = 0;
        words[k] = self.groups.len() as u32;
        k += 1;
        for g in &self.groups {
            words[k] = g.len() as u32;
            k += 1;
        }
        for g in &self.groups {
            let lanes: [fn(&Instance) -> f32; 5] = [|i| i.x, |i| i.y, |i| i.z, |i| i.rotation, |i| i.scale];
            for f in lanes {
                for i in g {
                    words[k] = f(i).to_bits();
                    k += 1;
                }
            }
            let padded = g.len().div_ceil(4);
            let flags = core::slice::from_raw_parts_mut(words[k..].as_mut_ptr() as *mut u8, padded * 4);
            flags.fill(0);
            for (n, i) in g.iter().enumerate() {
                flags[n] = i.flags;
            }
            k += padded;
            for i in g {
                words[k] = i.band;
                k += 1;
            }
            for i in g {
                words[k] = i.pos;
                k += 1;
            }
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
