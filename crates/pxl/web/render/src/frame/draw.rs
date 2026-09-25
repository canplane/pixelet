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
use xpute_runtime::mem::heap::{alloc, free};

/// Words an upload record holds: the handle, the block's address and its
/// length in bytes, and the vertex and index counts, which name the lanes.
pub const UPLOAD_WORDS: usize = 5;
/// Words a draw record holds: the handle, the vertex and index counts, the
/// index of its uniforms, and the pipeline.
pub const DRAW_WORDS: usize = 5;
/// Floats a draw's uniforms hold: its translation in the frame the camera
/// is written in (3), its visibility as the chunk lane encoded it (amount,
/// or 2 + amount for the complement), its opacity, and padding to a
/// 32-byte stride.
pub const UNIFORM_FLOATS: usize = 8;

/// The pipelines a draw names. A chunk draw's words are (its arena group
/// (gpu/arena.rs), box count, first box, uniform, pipeline): a chunk's mesh
/// is always in the arena, which the pipeline says, so the first word is the
/// group and never a block's handle. A decor draw's are (the decor block's
/// handle, the group — which kind and variant, in teaching order —, the
/// instance count, uniform, pipeline), its uniform carrying the group's
/// first lane word in place of the visibility.
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
/// zone's centre; the beacon as (0, night, 0, uniform, pipeline), its
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
/// Words a rewrite record holds: the handle, and the block's address and
/// length, whose bytes replace the buffer's.
pub const REWRITE_WORDS: usize = 3;

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
        if handle == 0 || self.rewrites.as_chunks::<REWRITE_WORDS>().0.iter().any(|r| r[0] == handle) {
            return;
        }
        self.rewrites.extend_from_slice(&[handle, at, bytes]);
    }

    /// Starts this frame's draws over.
    pub fn begin_frame(&mut self) {
        self.draws.clear();
        self.uniforms.clear();
    }

    /// A draw's translation: where `at` is against the frame's `origin`,
    /// narrowed once the difference is small.
    pub fn translation(at: V3, origin: V3) -> [f32; 3] {
        [(at[0] - origin[0]) as f32, (at[1] - origin[1]) as f32, (at[2] - origin[2]) as f32]
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
    pub fn draw_arena(&mut self, group: u32, first_box: u32, box_cnt: u32, translation: [f32; 3], visible: f32, unit: f32, z_step: f32, fine: bool) {
        let pipeline = if fine { PIPELINE_CHUNK } else { PIPELINE_CHUNK_COARSE };
        self.draw_in(pipeline, group, box_cnt, first_box, translation, visible, 1.0, unit, z_step);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], visible: f32, opacity: f32) {
        self.draw_in(pipeline, handle, vertex_cnt, index_cnt, translation, visible, opacity, 1.0, 1.0);
    }

    /// A draw whose boxes are measured in cells of `unit` units, their
    /// heights in steps of `z_step` from the floor in `translation`. The
    /// eighth float is the record's own padding: eight is two vectors, and a
    /// record of two vectors is the one a storage array cannot lay out two
    /// ways (chunk.wgsl's Draw).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_in(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], visible: f32, opacity: f32, unit: f32, z_step: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], visible, opacity, unit, z_step, 0.0]);
        self.draws.extend_from_slice(&[handle, vertex_cnt, index_cnt, uniform, pipeline]);
    }

    /// A decor group's instances: `offset` is the group's first lane word in
    /// the block (crates/pxl/web/kernel/src/layer/decor/build.rs's layout), exact in a float to 2^24 words,
    /// and `visible` the share of a dissolve they are drawn over, as a
    /// chunk's tier is (`draw_arena`).
    pub fn draw_decor(&mut self, handle: u32, group: u32, count: u32, offset: u32, translation: [f32; 3], visible: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms
            .extend_from_slice(&[translation[0], translation[1], translation[2], offset as f32, 1.0, visible, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, group, count, uniform, PIPELINE_DECOR]);
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
    /// Copies `len` bytes at `lane` of `mem` into the block, growing it
    /// from the heap first when it must; false when it refused, the
    /// block then gone.
    ///
    /// # Safety
    /// `lane` addresses `len` bytes in `mem`, which the heap's blocks are in.
    pub unsafe fn write(&mut self, draw: &mut DrawList, mem: *mut u8, lane: usize, len: u32) -> bool {
        let need = len.max(4);
        if need > self.bytes {
            self.free(draw);
            let bytes = need.next_power_of_two();
            let Some((at, bytes)) = alloc(bytes) else { return false };
            self.at = at;
            self.bytes = bytes;
        }
        core::ptr::copy_nonoverlapping(mem.add(lane), mem.add(self.at), len as usize);
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
