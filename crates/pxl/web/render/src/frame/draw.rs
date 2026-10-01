// pxl-render/frame/draw.rs

//! What the renderer draws, as lists: the blocks to put on the GPU and the
//! ones to let go, and each frame the draws, with what each needs beyond its
//! geometry. The scheduler decides the lists — which mesh, at what weight,
//! where — and the renderer (gpu/renderer.rs) executes them through the
//! host's WebGPU calls (gpu/api.rs) and decides nothing; a native host would
//! answer the same calls with wgpu.
//!
//! A block is named by a handle the kernel hands out when a tier lands; the
//! upload says where the block lies in the memory and how long it is, and
//! the renderer reads the lanes by the mesher's layout from the vertex count.
//! Uploads and frees accumulate until the renderer takes them, whatever calls
//! made them (a tick, a build now); the draws are the last tick's. A free
//! cancels an upload not yet taken: the block is released the moment the
//! free is listed, and may be someone else's by the time the list runs.

use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

/// Words an upload record holds: the handle, the block's address and its
/// length in bytes, and the vertex and index counts, which name the lanes.
pub const UPLOAD_WORDS: usize = 5;
/// Words a draw record holds: the handle, the vertex and index counts, the
/// index of its uniforms, and the pipeline.
pub const DRAW_WORDS: usize = 5;
/// Floats a draw's uniforms hold: its translation in the frame the camera
/// is written in (3), a fourth its pipeline reads its own way (an actor's
/// turn, the beacon's spin; nothing for a chunk), its opacity, and the rest
/// to a 32-byte stride.
pub const UNIFORM_FLOATS: usize = 8;

/// The pipelines a draw names. A chunk draw's words are (its arena group
/// (gpu/arena.rs), box count, first box, uniform, pipeline): a chunk's mesh
/// is always in the arena, which the pipeline says, so the first word is the
/// group and never a block's handle. A decor draw's are (the decor block's
/// handle, the group — which kind and variant, in teaching order —, the
/// instance count, uniform, pipeline), its uniform carrying the group's
/// first lane word as its fourth float.
pub const PIPELINE_CHUNK: u32 = 0;
pub const PIPELINE_DECOR: u32 = 1;
/// The hover ghost: a box through the chunk shader, or one decor instance
/// through decor's, half transparent; the box biased toward the eye, more so
/// when it sits in a surface rather than clear of one.
pub const PIPELINE_GHOST_BOX: u32 = 2;
pub const PIPELINE_GHOST_BOX_COPLANAR: u32 = 3;
pub const PIPELINE_GHOST_DECOR: u32 = 4;
/// Words a model vertex takes in a model's mesh block: position (3), normal
/// (3), UV (2), joints (4, as u32), weights (4); the indices follow the
/// vertex lanes.
pub const MODEL_VERTEX_WORDS: u32 = 16;
/// An actor (crates/pxl/web/kernel/src/layer/model/store.rs): words (the model's handle, the joint palette's
/// handle, the model id, uniform, pipeline), its uniform's fourth float the
/// turn about Z.
pub const PIPELINE_SKINNED: u32 = 5;
/// The location puck (crates/pxl/web/kernel/src/layer/marker/location_puck.rs): the zone's instances as (the zone block's
/// handle, the instance count, 0, uniform, pipeline), translated to the
/// zone's center; the beacon as (0, night, 0, uniform, pipeline), its
/// uniform's fourth float its spin.
pub const PIPELINE_PUCK_ZONE: u32 = 6;
pub const PIPELINE_PUCK_BEACON: u32 = 7;
/// The badges (crates/pxl/web/kernel/src/layer/billboard/field.rs): (the block's handle, the count, 0, uniform,
/// pipeline), translated to the placement's origin.
pub const PIPELINE_BADGE: u32 = 8;
/// Line segments over everything: handle, vertex count.
pub const PIPELINE_DEBUG: u32 = 9;
/// A chunk mesh whose boxes are the coarse four-word format: the same shader
/// and the same groups as PIPELINE_CHUNK over a vertex layout of four words
/// rather than six, because a pipeline's stride is fixed and the two formats
/// do not share one (frame/shader/record.rs's chunk_lanes).
pub const PIPELINE_CHUNK_COARSE: u32 = 32;
/// Decor drawn as its variants' sprites, where a variant is too small on the
/// screen for its model: the decor block's instances, a quad each
/// (gpu/pass/impostor.rs).
pub const PIPELINE_IMPOSTOR: u32 = 34;
/// Words a rewrite record holds: the handle, the block's address, and the
/// byte offset into it and length of the bytes that replace the buffer's
/// there.
pub const REWRITE_WORDS: usize = 4;

#[derive(Default)]
pub struct DrawList {
    uploads: Vec<u32>,
    rewrites: Vec<u32>,
    frees: Vec<u32>,
    draws: Vec<u32>,
    uniforms: Vec<f32>,
    next_handle: u32,
    /// The uploads, rewrites and frees the renderer last took, their lanes
    /// kept to be filled again rather than made anew.
    taken: [Vec<u32>; 3],
}

/// The lists a frame is drawn from, borrowed until the next write: the
/// blocks to put on the device, rewrite and let go since the last take, and
/// the last tick's draws and their uniforms.
pub struct Lists<'a> {
    pub uploads: &'a [u32],
    pub rewrites: &'a [u32],
    pub frees: &'a [u32],
    pub draws: &'a [u32],
    pub uniforms: &'a [f32],
}

impl DrawList {
    /// A handle for a block the shell is to upload.
    pub fn upload(&mut self, at: u32, bytes: u32, vertex_cnt: u32, index_cnt: u32) -> u32 {
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let handle = self.next_handle;
        self.uploads.extend_from_slice(&[handle, at, bytes, vertex_cnt, index_cnt]);
        handle
    }

    /// The block behind `handle` is gone from the shell's memory, or about to
    /// be; 0 names no block.
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
        let mut k = 0;
        while k < self.uploads.len() {
            if self.uploads[k] == handle {
                self.uploads.drain(k..k + UPLOAD_WORDS);
                // An upload never taken needs no free either.
                return;
            }
            k += UPLOAD_WORDS;
        }
        self.frees.push(handle);
    }

    /// The block behind `handle` changed: its bytes go to the GPU again,
    /// into the buffer it already has. A rewrite already listed stands.
    pub fn rewrite(&mut self, handle: u32, at: u32, bytes: u32) {
        self.rewrite_part(handle, at, 0, bytes);
    }

    /// `bytes` of the block behind `handle` changed, `from` bytes into it:
    /// those go to the GPU again, at the same place in its buffer. A rewrite
    /// of the same part already listed stands.
    pub fn rewrite_part(&mut self, handle: u32, at: u32, from: u32, bytes: u32) {
        if handle == 0 || self.rewrites.as_chunks::<REWRITE_WORDS>().0.iter().any(|r| r[0] == handle && r[2] == from) {
            return;
        }
        self.rewrites.extend_from_slice(&[handle, at, from, bytes]);
    }

    /// Starts this frame's draws over.
    pub fn begin_frame(&mut self) {
        self.draws.clear();
        self.uniforms.clear();
    }

    /// A draw's translation: where `at` is against the `eye`, both in
    /// units, narrowed once the difference is small. Every point the GPU
    /// holds is relative to the eye, so what it rounds is as small as the
    /// point is near, and what is near the eye is held finest.
    pub fn translation(at: V3, eye: V3) -> [f32; 3] {
        [(at[0] - eye[0]) as f32, (at[1] - eye[1]) as f32, (at[2] - eye[2]) as f32]
    }

    /// An actor's draw: a draw_as whose sixth uniform float is the model's scale.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scaled(&mut self, pipeline: u32, handle: u32, a: u32, b: u32, translation: [f32; 3], fourth: f32, opacity: f32, scale: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], fourth, opacity, scale, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, a, b, uniform, pipeline]);
    }

    /// A draw whose record names a pipeline other than the chunk's.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_as(&mut self, pipeline: u32, handle: u32, a: u32, b: u32, translation: [f32; 3], fourth: f32, opacity: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], fourth, opacity, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, a, b, uniform, pipeline]);
    }

    /// A chunk mesh in arena group `group`, `box_cnt` boxes from `first_box`.
    /// `unit`: the cell a box of this mesh is measured in, in units, and
    /// `z_step` what one step of its height is; `fine` says which of the two
    /// box formats the group holds, which is the pipeline and reaches the
    /// shader as the entry point it is drawn at. The mesh's own floor is in
    /// `translation`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_arena(&mut self, group: u32, first_box: u32, box_cnt: u32, translation: [f32; 3], unit: f32, z_step: f32, fine: bool) {
        let pipeline = if fine { PIPELINE_CHUNK } else { PIPELINE_CHUNK_COARSE };
        self.draw_in(pipeline, group, box_cnt, first_box, translation, 1.0, unit, z_step);
    }

    /// A draw whose boxes are measured in cells of `unit` units, their
    /// heights in steps of `z_step` from the floor in `translation`. The
    /// eighth float is the record's own padding: eight is two vectors, and a
    /// record of two vectors is the one a storage array cannot lay out two
    /// ways (chunk.wgsl's Draw).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_in(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], opacity: f32, unit: f32, z_step: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], 0.0, opacity, unit, z_step, 0.0]);
        self.draws.extend_from_slice(&[handle, vertex_cnt, index_cnt, uniform, pipeline]);
    }

    /// Instances of decor group `group` in block `handle`, under `pipeline`
    /// (the model, its ghost, or its sprite): `count` of them, each named by
    /// a word of the index list at word `index` of the block, read from the
    /// group's lanes at word `offset`, `stride` instances long
    /// (crates/pxl/web/kernel/src/layer/decor/build.rs's layout). Words are
    /// exact in a float to 2^24.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_instances(&mut self, pipeline: u32, handle: u32, group: u32, count: u32, lanes: [u32; 3], translation: [f32; 3], opacity: f32) {
        let [offset, stride, index] = lanes;
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms
            .extend_from_slice(&[translation[0], translation[1], translation[2], offset as f32, opacity, stride as f32, index as f32, 0.0]);
        self.draws.extend_from_slice(&[handle, group, count, uniform, pipeline]);
    }

    /// The draws the last frame listed.
    pub fn draw_count(&self) -> u32 {
        (self.draws.len() / DRAW_WORDS) as u32
    }

    /// Hands the lists to the renderer. Uploads, rewrites and frees start
    /// fresh; the draws stay until the next frame writes them.
    pub fn take(&mut self) -> Lists<'_> {
        let [uploads, rewrites, frees] = &mut self.taken;
        core::mem::swap(uploads, &mut self.uploads);
        core::mem::swap(rewrites, &mut self.rewrites);
        core::mem::swap(frees, &mut self.frees);
        self.uploads.clear();
        self.rewrites.clear();
        self.frees.clear();
        Lists {
            uploads,
            rewrites,
            frees,
            draws: &self.draws,
            uniforms: &self.uniforms,
        }
    }
}

/// A heap block the renderer holds a copy of, written whole
/// each time by whoever owns it — the badges, the gizmos: grown to the
/// next power of two when the write outgrows it, since a placement's count
/// wobbles by a few; uploaded once, rewritten after.
#[derive(Default)]
pub struct Held {
    pub at: usize,
    pub bytes: u32,
    pub handle: u32,
}

impl Held {
    /// Copies `lane` into the block, growing it from the heap first when it
    /// must; false when it refused, the block then gone.
    pub fn write(&mut self, draw: &mut DrawList, lane: &[u8]) -> bool {
        let need = (lane.len() as u32).max(4);
        if need > self.bytes {
            self.free(draw);
            let bytes = need.next_power_of_two();
            let Some((at, bytes)) = alloc(bytes) else { return false };
            self.at = at;
            self.bytes = bytes;
        }
        // SAFETY: the block is the heap's, `self.bytes` long, and at least
        // `lane` long.
        unsafe { core::ptr::copy_nonoverlapping(lane.as_ptr(), ptr_at::<u8>(self.at), lane.len()) };
        if self.handle == 0 {
            self.handle = draw.upload(self.at as u32, self.bytes, 0, 0);
        } else {
            draw.rewrite(self.handle, self.at as u32, need);
        }
        true
    }

    /// Lets the block go, on the renderer and in the heap.
    pub fn free(&mut self, draw: &mut DrawList) {
        if self.bytes > 0 {
            draw.free(self.handle);
            // SAFETY: the block this lane took and has not given back.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
            self.handle = 0;
        }
    }
}
