// pxl-render/frame/draw.rs

//! Draw lists the kernel fills and the renderer executes without deciding.
//! A free cancels its block's pending upload and rewrites: the block may be
//! reused before the list runs.

use crate::frame::shader::record::reads_shadow_map;
use crate::frame::shader::word::DRAW_FLOATS;
use crate::gpu::api::in_flight;
use pxl_content::generation::{TALLEST, WIDEST};
use pxl_engine::light;
use pxl_spec::space::coord::q3::{from_real, ONE, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::between;
use xpute_guest::global::Global;
use xpute_guest::mem;
use xpute_guest::mem::heap::{alloc, free};
use xpute_kit::status::errno::Errno;

/// Upload record: handle, address, byte length, vertex count, index count.
pub const UPLOAD_WORDS: usize = 5;
/// Draw record: handle, two pipeline-specific words, uniform index, pipeline.
pub const DRAW_WORDS: usize = 5;

pub fn uniform_of<'a>(uniforms: &'a [f32], d: &[u32; DRAW_WORDS]) -> &'a [f32] {
    &uniforms[d[3] as usize * DRAW_FLOATS..][..DRAW_FLOATS]
}

/// Where a draw stands in the world, free of the eye: its Q3, and the f32
/// remainder `r` of a position finer than Q3, zero for one on it. A record
/// holds it, the frame the eye's Q3 (word.rs `frame::EYE`), and a shader
/// takes the one against the other (frame.wgsl `from_eye`), as `from_eye`
/// here does: the i32 difference first, wrapping, then
/// `f32(q - eye) / 256 + r`.
///
/// - The split is a floor: `q = floor(v * 256)` (pxl-spec `q3::from_real`),
///   narrowed from the i64 with `as i32`, a truncation mod 2^32, and
///   `r = v - q / 256`, so `r` lies in [0, 1/256) on negative coordinates
///   too. The position passes no clamp of the world's (pxl-spec `world::at`,
///   `world::origin`): a preview stands 100,000 units under the eye.
/// - X goes round: the wrapping difference is the short way round, the
///   round 2^32 Q3, so a record on any lap draws where its copy nearest the
///   eye does.
/// - Y and Z do not go round: their wrapping difference is the true one
///   while it is under 2^31 Q3, and a drawn record lies within the draw
///   distance (pxl-kernel control/param.rs `MAX_DIST`, 2^17 units, 2^25 Q3)
///   of the eye.
/// - On Q3 (`r` zero) the result is the eye-relative translation the
///   records held before, `f32((at - eye) as f64)`: the f64 difference is
///   exact and narrowed once, and `f32(q - eye)` rounds the same exact
///   difference once before a power-of-two scale, which is exact. A device
///   that rounds an i32 to f32 other than to nearest differs by an ulp past
///   2^24 Q3, where WGSL lets it pick either neighbour.
/// - With a remainder the result is within an ulp of that translation, or
///   within an ulp and 2^-33 units: below 2^24 Q3 `r`, rounded to f32 once,
///   is added in a second rounding, which can land on the other side of a
///   tie; past it the difference is rounded before `r` is added; and where
///   the difference is a step or two and `r` cancels it, the result is
///   smaller than `r` and keeps `r`'s rounding, half an ulp of 1/256, 2^-33
///   units. Only what is drawn takes this; what decides anything
///   (selection, collision, visibility, culling, LOD, the shadow's
///   receivers) reads Q3 or f64.
/// - Nothing decided on the GPU reads a remainder: decor's gate
///   (decor_gate.wgsl) reads decor's anchor, on Q3, and the light's
///   receivers read each `Stand` rounded outward from Q3
///   (`Stand::from_eye`). The screen tiles a draw marks (tile_mark.wgsl) are
///   where it is drawn, by the same placing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub q: Q3,
    pub r: [f32; 3],
}

/// Units as a position's Q3, X, Y and Z alike: the floor's i64 truncated
/// to an i32, never an f64's saturating `as i32`.
pub fn q3_of(v: V3) -> Q3 {
    v.map(|x| from_real(x) as i32)
}

/// Units as a position's Q3 rounded up, as `q3_of` rounds down.
fn q3_up(v: V3) -> Q3 {
    v.map(|x| from_real(-x).wrapping_neg() as i32)
}

impl Position {
    /// None of a draw's own: its instances each hold theirs, as a marker
    /// set's do (word.rs `MARKER_INSTANCE_WORDS`).
    pub const NONE: Position = Position {
        q: [0; 3],
        r: [0.0; 3],
    };

    pub fn of(v: V3) -> Position {
        let q = q3_of(v);
        let wide = v.map(from_real);
        Position {
            q,
            r: core::array::from_fn(|k| (v[k] - wide[k] as f64 / ONE as f64) as f32),
        }
    }

    /// Units from the eye whose Q3 is `eye`, as a shader takes them.
    pub fn from_eye(&self, eye: Q3) -> [f32; 3] {
        core::array::from_fn(|k| {
            self.q[k].wrapping_sub(eye[k]) as f32 * (1.0 / ONE as f32) + self.r[k]
        })
    }

    /// A draw's, from its uniform (`uniform_of`).
    pub fn in_uniform(u: &[f32]) -> Position {
        Position {
            q: [0, 1, 2].map(|k| u[k].to_bits() as i32),
            r: [u[8], u[9], u[10]],
        }
    }

    /// Its Q3 in a uniform's floats, bit for bit.
    fn q_words(&self) -> [f32; 3] {
        self.q.map(|v| f32::from_bits(v as u32))
    }
}

/// Box draw: (upload handle, box count, first box, uniform, pipeline).
/// Decor draw: (upload handle, group, instance count, uniform, pipeline); the
/// uniform carries the group's first column word, stride and index-list start.
pub const PIPELINE_DECOR: u32 = 0;
/// Half transparent, at the depth of the faces it lies on, which stand
/// behind it (screen_ray.wgsl `behind`).
pub const PIPELINE_GHOST_BOX: u32 = 1;
pub const PIPELINE_GHOST_DECOR: u32 = 2;
/// Position 3, normal 3, UV 2, joints 4 (u32), weights 4; indices follow the vertex columns.
pub const MODEL_VERTEX_WORDS: u32 = 16;
/// (model handle, joint palette handle, model id, uniform, pipeline); uniform
/// float 4 is the turn about Z.
pub const PIPELINE_SKINNED: u32 = 3;
/// Zone: (zone upload handle, instance count, 0, uniform, pipeline). Beacon:
/// (0, night, 0, uniform, pipeline), uniform float 4 its spin.
pub const PIPELINE_LOCATION_ZONE: u32 = 4;
pub const PIPELINE_LOCATION_BEACON: u32 = 5;
/// (upload handle, count, 0, uniform, pipeline).
pub const PIPELINE_MARKER: u32 = 6;
/// Line segments: handle, line vertex count, face vertex count.
pub const PIPELINE_DEBUG: u32 = 7;
/// Decor drawn as impostors where a variant is too small on screen for its model.
pub const PIPELINE_IMPOSTOR: u32 = 8;

/// Every pipeline a draw record names, in the order a frame's draw stats count
/// them (gpu/renderer.rs `FrameStat::draws`): what the list draws is these.
pub const DRAW_PIPELINES: [u32; 9] = [
    PIPELINE_DECOR,
    PIPELINE_GHOST_BOX,
    PIPELINE_GHOST_DECOR,
    PIPELINE_SKINNED,
    PIPELINE_LOCATION_ZONE,
    PIPELINE_LOCATION_BEACON,
    PIPELINE_MARKER,
    PIPELINE_DEBUG,
    PIPELINE_IMPOSTOR,
];

/// A draw record's (draws, triangles), by its pipeline's place in `DRAW_PIPELINES`.
pub type DrawStats = [[f64; 2]; DRAW_PIPELINES.len()];

/// `pipeline`'s place in `DRAW_PIPELINES`; a draw naming another is a bug.
pub fn stats_slot(pipeline: u32) -> usize {
    use xpute_kit::status::bug::OrBug;
    DRAW_PIPELINES
        .iter()
        .position(|&p| p == pipeline)
        .or_bug(Errno::EINVAL)
}
/// Rewrite record: handle, address, byte offset, length.
pub const REWRITE_WORDS: usize = 4;

/// The uploads' and the frees' arrays in `.fixed.uploads` (pxl-render
/// gpu/upload.rs `set_uploads`), each twice, the one written and the one
/// the renderer took: room for as many records as uploads live, since a
/// turn's uploads become live and its frees were.
struct ListArrays {
    uploads: [&'static mut [u32]; 2],
    frees: [&'static mut [u32]; 2],
}

static LIST_ARRAYS: Global<ListArrays> = Global::new();

fn list_arrays() -> &'static mut ListArrays {
    LIST_ARRAYS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}

/// The lists' arrays' bytes, for `live` uploads.
pub const fn lists_bytes(live: usize) -> u32 {
    (2 * (UPLOAD_WORDS + 1) * live * 4) as u32
}

/// # Safety
/// `at` must address `lists_bytes(live)`, 4-aligned, the lists' alone for
/// as long as the module runs.
pub unsafe fn set_lists(at: usize, live: usize) {
    // SAFETY: the caller's range, one array after another.
    let arr = |from: usize, words: usize| unsafe {
        core::slice::from_raw_parts_mut(mem::at::<u32>(at + from * 4), words)
    };
    let (u, f) = (UPLOAD_WORDS * live, live);
    LIST_ARRAYS.set(ListArrays {
        uploads: [arr(0, u), arr(u, u)],
        frees: [arr(2 * u, f), arr(2 * u + f, f)],
    });
}

#[derive(Default)]
pub struct DrawList {
    /// The lists' side written, and how much of each is.
    side: usize,
    uploads: usize,
    frees: usize,
    rewrites: Vec<u32>,
    draws: Vec<u32>,
    /// The frame's uniforms, written into whichever of the two the host
    /// does not hold (gpu/api.rs `in_flight`): the renderer hands them where
    /// they lie.
    uniforms: Vec<f32>,
    other_uniforms: Vec<f32>,
    next_handle: u32,
    taken: Vec<u32>,
    /// The box each drawn thing that reads the shadow map stands within (`Stand`).
    stands: Vec<Stand>,
}

/// The box a drawn thing that reads the shadow map stands within, in the world, free of the
/// eye: its lowest corner's Q3 rounded down and its highest's rounded up, so it holds the box
/// asked. A receiver of the map stands by the rule every receiver does (pxl-engine
/// `light::STANDS_AT`, `Light::box_tiles`), and which of the light's tiles it stands on is a
/// decision, so the box stays exact: Q3, taken against the eye on the CPU in integers and
/// rounded outward to the f32 the light's passes read (`from_eye`). X goes round as a
/// `Position`'s does; a box is no wider than the draw distance, so its corners' differences
/// are the true ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand(pub [Q3; 2]);

impl Stand {
    /// The box from `lo` to `hi`, in units, held whole.
    pub fn of(lo: V3, hi: V3) -> Stand {
        Stand([q3_of(lo), q3_up(hi)])
    }

    /// The box about the ball a thing standing at `at` stands within: about its middle, as
    /// wide as decor is tall and wide (pxl-render decor_ball.wgsl, which reads a placed
    /// decor's alike).
    pub fn ball(at: V3) -> Stand {
        let r = f64::from(TALLEST + WIDEST);
        let middle = [at[0], at[1], at[2] + f64::from(TALLEST) * 0.5];
        Stand::of(middle.map(|v| v - r), middle.map(|v| v + r))
    }

    /// Its corners in units from the eye whose Q3 is `eye`, as the light's passes read them:
    /// the differences in integers, X the short way round, each rounded once and outward, the
    /// low corner down and the high up, so the box read holds the box stood.
    pub fn from_eye(&self, eye: Q3) -> [[f32; 3]; 2] {
        let [lo, hi] = self.0;
        let from = between(&eye, &lo);
        let across = between(&lo, &hi);
        let step = |d: i64, up: bool| {
            let f = d as f32;
            let v = match (f as f64).partial_cmp(&(d as f64)) {
                Some(core::cmp::Ordering::Greater) if !up => f.next_down(),
                Some(core::cmp::Ordering::Less) if up => f.next_up(),
                _ => f,
            };
            v * (1.0 / ONE as f32)
        };
        [
            from.map(|d| step(d, false)),
            core::array::from_fn(|k| step(from[k] + across[k], true)),
        ]
    }

    /// Grown to hold `b` too, each axis about its own low corner, so X goes round as the
    /// corners do.
    fn hold(&mut self, b: &Stand) {
        let [base, top] = self.0;
        let [lo, hi]: [Q3; 2] = [0, 1].map(|end| {
            core::array::from_fn(|k| {
                let off = |v: i32| v.wrapping_sub(base[k]);
                let v = if end == 0 {
                    off(b.0[0][k]).min(0)
                } else {
                    off(top[k]).max(off(b.0[1][k]))
                };
                base[k].wrapping_add(v)
            })
        });
        self.0 = [lo, hi];
    }
}

pub struct Lists<'a> {
    pub uploads: &'a [u32],
    pub rewrites: &'a [u32],
    pub frees: &'a [u32],
    pub draws: &'a [u32],
    /// Handed where they lie (`DrawList::begin_frame`).
    pub uniforms: &'a [f32],
    /// The box each drawn thing that reads the shadow map stands within, as its draw is
    /// recorded (`Stand`).
    pub stands: &'a [Stand],
}

impl DrawList {
    pub fn upload(&mut self, at: u32, bytes: u32, vertex_cnt: u32, idx_cnt: u32) -> u32 {
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let handle = self.next_handle;
        let arr = &mut list_arrays().uploads[self.side];
        xpute_kit::ensure!(self.uploads + UPLOAD_WORDS <= arr.len(), ENOSPC, arr.len());
        arr[self.uploads..self.uploads + UPLOAD_WORDS]
            .copy_from_slice(&[handle, at, bytes, vertex_cnt, idx_cnt]);
        self.uploads += UPLOAD_WORDS;
        handle
    }

    /// `handle` 0 names no upload.
    pub fn free(&mut self, handle: u32) {
        if handle == 0 {
            return;
        }
        let mut k = 0;
        while k < self.rewrites.len() {
            if self.rewrites[k] == handle {
                self.rewrites.drain(k..k + REWRITE_WORDS);
            } else {
                k += REWRITE_WORDS;
            }
        }
        let (uploads, frees) = (
            &mut list_arrays().uploads[self.side],
            &mut list_arrays().frees[self.side],
        );
        let mut k = 0;
        while k < self.uploads {
            if uploads[k] == handle {
                uploads.copy_within(k + UPLOAD_WORDS..self.uploads, k);
                self.uploads -= UPLOAD_WORDS;
                // An upload never taken needs no free either.
                return;
            }
            k += UPLOAD_WORDS;
        }
        xpute_kit::ensure!(self.frees < frees.len(), ENOSPC, frees.len());
        frees[self.frees] = handle;
        self.frees += 1;
    }

    pub fn rewrite(&mut self, handle: u32, at: u32, bytes: u32) {
        self.rewrite_part(handle, at, 0, bytes);
    }

    /// One rewrite a part of an upload, however often it is asked before the
    /// list is taken: its bytes are read then, at the latest place asked,
    /// as many as the most asked.
    pub fn rewrite_part(&mut self, handle: u32, at: u32, from: u32, bytes: u32) {
        if handle == 0 {
            return;
        }
        if let Some(r) = self
            .rewrites
            .as_chunks_mut::<REWRITE_WORDS>()
            .0
            .iter_mut()
            .find(|r| r[0] == handle && r[2] == from)
        {
            (r[1], r[3]) = (at, r[3].max(bytes));
            return;
        }
        self.rewrites.extend_from_slice(&[handle, at, from, bytes]);
    }

    pub fn begin_frame(&mut self) {
        if in_flight(&self.uniforms) {
            core::mem::swap(&mut self.uniforms, &mut self.other_uniforms);
        }
        // One frame a hand-off hands them, so the host holds one at most.
        xpute_kit::ensure!(!in_flight(&self.uniforms), EBUSY);
        self.draws.clear();
        self.uniforms.clear();
        self.stands.clear();
    }

    /// Where the draw of `pipeline` stands: the box `stand`, which every draw whose shader reads
    /// the shadow map gives and no other does (record.rs `reads_shadow_map`), so whatever reads
    /// the map stands. Past `light::STANDS` the box is taken into the last.
    fn stand(&mut self, pipeline: u32, stand: Option<Stand>) {
        xpute_kit::ensure!(
            reads_shadow_map(pipeline) == stand.is_some(),
            EINVAL,
            pipeline
        );
        let Some(b) = stand else { return };
        let full = self.stands.len() == light::STANDS;
        match self.stands.last_mut() {
            Some(last) if full => last.hold(&b),
            _ => self.stands.push(b),
        }
    }

    /// Whether `other` lists the same draws with the same uniforms, bit for bit, and the same
    /// stands: what the renderer writes of the one over the other is nothing (gpu/read.rs
    /// `Written`), and the light's receivers read the same (gpu/ray/cast.rs).
    pub fn same_draws(&self, other: &DrawList) -> bool {
        let bits = |u: &[f32]| u.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        self.draws == other.draws
            && bits(&self.uniforms) == bits(&other.uniforms)
            && self.stands == other.stands
    }

    /// A draw's uniform, its index: `at`'s Q3, the five words its pipeline
    /// reads past it, and `at`'s remainder (`Position`).
    fn put(&mut self, at: Position, words: [f32; 5]) -> u32 {
        let uniform = (self.uniforms.len() / DRAW_FLOATS) as u32;
        let [q0, q1, q2] = at.q_words();
        let [r0, r1, r2] = at.r;
        let [w3, w4, w5, w6, w7] = words;
        self.uniforms
            .extend_from_slice(&[q0, q1, q2, w3, w4, w5, w6, w7, r0, r1, r2, 0.0]);
        uniform
    }

    /// An actor's draw: a draw_as whose sixth uniform float is the model's scale.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_scaled(
        &mut self,
        pipeline: u32,
        handle: u32,
        a: u32,
        b: u32,
        at: Position,
        fourth: f32,
        opacity: f32,
        scale: f32,
        stand: Option<Stand>,
    ) {
        self.stand(pipeline, stand);
        let uniform = self.put(at, [fourth, opacity, scale, 0.0, 0.0]);
        self.draws
            .extend_from_slice(&[handle, a, b, uniform, pipeline]);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_as(
        &mut self,
        pipeline: u32,
        handle: u32,
        a: u32,
        b: u32,
        at: Position,
        fourth: f32,
        opacity: f32,
        stand: Option<Stand>,
    ) {
        self.stand(pipeline, stand);
        let uniform = self.put(at, [fourth, opacity, 0.0, 0.0, 0.0]);
        self.draws
            .extend_from_slice(&[handle, a, b, uniform, pipeline]);
    }

    /// The eighth float pads the record's first two vec4s, and the twelfth
    /// its remainder's: whole vec4s are the one layout a storage array
    /// cannot read two ways.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_in(
        &mut self,
        pipeline: u32,
        handle: u32,
        vertex_cnt: u32,
        idx_cnt: u32,
        at: Position,
        opacity: f32,
        unit: f32,
        z_step: f32,
        stand: Option<Stand>,
    ) {
        self.stand(pipeline, stand);
        let uniform = self.put(at, [0.0, opacity, unit, z_step, 0.0]);
        self.draws
            .extend_from_slice(&[handle, vertex_cnt, idx_cnt, uniform, pipeline]);
    }

    /// Words are exact in an f32 up to 2^24.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_instances(
        &mut self,
        pipeline: u32,
        handle: u32,
        group: u32,
        cnt: u32,
        columns: [u32; 3],
        at: Position,
        opacity: f32,
        stand: Option<Stand>,
    ) {
        let [off, stride, idx] = columns;
        self.stand(pipeline, stand);
        let uniform = self.put(at, [off as f32, opacity, stride as f32, idx as f32, 0.0]);
        self.draws
            .extend_from_slice(&[handle, group, cnt, uniform, pipeline]);
    }

    /// Uploads, rewrites and frees start fresh, on the lists' other side;
    /// draws stay until the next frame.
    pub fn take(&mut self) -> Lists<'_> {
        let (side, uploads, frees) = (self.side, self.uploads, self.frees);
        (self.side, self.uploads, self.frees) = (1 - side, 0, 0);
        core::mem::swap(&mut self.taken, &mut self.rewrites);
        self.rewrites.clear();
        let l = list_arrays();
        Lists {
            uploads: &l.uploads[side][..uploads],
            rewrites: &self.taken,
            frees: &l.frees[side][..frees],
            draws: &self.draws,
            uniforms: &self.uniforms,
            stands: &self.stands,
        }
    }
}

/// A heap block the renderer mirrors, written whole each time and grown to the
/// next power of two.
#[derive(Default)]
pub struct MirroredBlock {
    pub at: usize,
    pub bytes: u32,
    pub handle: u32,
}

impl MirroredBlock {
    /// False when the heap refused; the block is then gone.
    pub fn write(&mut self, draw: &mut DrawList, data: &[u8]) -> bool {
        let need = (data.len() as u32).max(4);
        if need > self.bytes {
            self.free(draw);
            let bytes = need.next_power_of_two();
            let Some((at, bytes)) = alloc(bytes) else {
                return false;
            };
            self.at = at;
            self.bytes = bytes;
        }
        // SAFETY: the block is the heap's, `self.bytes` long, and at least
        // `data` long.
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), mem::at::<u8>(self.at), data.len());
        };
        if self.handle == 0 {
            self.handle = draw.upload(self.at as u32, self.bytes, 0, 0);
        } else {
            draw.rewrite(self.handle, self.at as u32, need);
        }
        true
    }

    pub fn free(&mut self, draw: &mut DrawList) {
        if self.bytes > 0 {
            draw.free(self.handle);
            // SAFETY: the block this write took and has not given back.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
            self.handle = 0;
        }
    }
}

#[cfg(test)]
#[path = "draw.test.rs"]
mod tests;
