// pxl-render/frame/draw.rs

//! Draw lists the kernel fills and the renderer executes without deciding.
//! A free cancels its block's pending upload and rewrites: the block may be
//! reused before the list runs.

use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

/// Upload record: handle, address, byte length, vertex count, index count.
pub const UPLOAD_WORDS: usize = 5;
/// Draw record: handle or arena group, two pipeline-specific words, uniform index, pipeline.
pub const DRAW_WORDS: usize = 5;
/// Draw uniforms: eye-relative translation (3), a pipeline-specific float,
/// opacity, padding to 32 bytes.
pub const UNIFORM_FLOATS: usize = 8;

/// Chunk draw: (arena group, box count, first box, uniform, pipeline).
/// Decor draw: (block handle, group, instance count, uniform, pipeline); the
/// uniform carries the group's first lane word, stride and index-list start.
pub const PIPELINE_CHUNK: u32 = 0;
pub const PIPELINE_DECOR: u32 = 1;
/// Half transparent and biased toward the eye.
pub const PIPELINE_GHOST_BOX: u32 = 2;
pub const PIPELINE_GHOST_BOX_COPLANAR: u32 = 3;
pub const PIPELINE_GHOST_DECOR: u32 = 4;
/// Position 3, normal 3, UV 2, joints 4 (u32), weights 4; indices follow the vertex lanes.
pub const MODEL_VERTEX_WORDS: u32 = 16;
/// (model handle, joint palette handle, model id, uniform, pipeline); uniform
/// float 4 is the turn about Z.
pub const PIPELINE_SKINNED: u32 = 5;
/// Zone: (zone block handle, instance count, 0, uniform, pipeline). Beacon:
/// (0, night, 0, uniform, pipeline), uniform float 4 its spin.
pub const PIPELINE_PUCK_ZONE: u32 = 6;
pub const PIPELINE_PUCK_BEACON: u32 = 7;
/// (block handle, count, 0, uniform, pipeline).
pub const PIPELINE_BADGE: u32 = 8;
/// Line segments over everything: handle, vertex count.
pub const PIPELINE_DEBUG: u32 = 9;
/// The coarse four-word box format; its own pipeline because a pipeline's
/// vertex stride is fixed.
pub const PIPELINE_CHUNK_COARSE: u32 = 32;
/// Decor drawn as sprites where a variant is too small on screen for its model.
pub const PIPELINE_IMPOSTOR: u32 = 34;
/// Rewrite record: handle, address, byte offset, length.
pub const REWRITE_WORDS: usize = 4;

#[derive(Default)]
pub struct DrawList {
    uploads: Vec<u32>,
    rewrites: Vec<u32>,
    frees: Vec<u32>,
    draws: Vec<u32>,
    uniforms: Vec<f32>,
    next_handle: u32,
    taken: [Vec<u32>; 3],
}

pub struct Lists<'a> {
    pub uploads: &'a [u32],
    pub rewrites: &'a [u32],
    pub frees: &'a [u32],
    pub draws: &'a [u32],
    pub uniforms: &'a [f32],
}

impl DrawList {
    pub fn upload(&mut self, at: u32, bytes: u32, vertex_cnt: u32, index_cnt: u32) -> u32 {
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let handle = self.next_handle;
        self.uploads.extend_from_slice(&[handle, at, bytes, vertex_cnt, index_cnt]);
        handle
    }

    /// `handle` 0 names no block.
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

    pub fn rewrite(&mut self, handle: u32, at: u32, bytes: u32) {
        self.rewrite_part(handle, at, 0, bytes);
    }

    pub fn rewrite_part(&mut self, handle: u32, at: u32, from: u32, bytes: u32) {
        if handle == 0 || self.rewrites.as_chunks::<REWRITE_WORDS>().0.iter().any(|r| r[0] == handle && r[2] == from) {
            return;
        }
        self.rewrites.extend_from_slice(&[handle, at, from, bytes]);
    }

    pub fn begin_frame(&mut self) {
        self.draws.clear();
        self.uniforms.clear();
    }

    /// Eye-relative, narrowed to f32 after the subtraction, so precision is finest near the eye.
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

    #[allow(clippy::too_many_arguments)]
    pub fn draw_as(&mut self, pipeline: u32, handle: u32, a: u32, b: u32, translation: [f32; 3], fourth: f32, opacity: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], fourth, opacity, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, a, b, uniform, pipeline]);
    }

    /// `fine` picks the box format and so the pipeline; the mesh's floor is in `translation`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_arena(&mut self, group: u32, first_box: u32, box_cnt: u32, translation: [f32; 3], unit: f32, z_step: f32, fine: bool) {
        let pipeline = if fine { PIPELINE_CHUNK } else { PIPELINE_CHUNK_COARSE };
        self.draw_in(pipeline, group, box_cnt, first_box, translation, 1.0, unit, z_step);
    }

    /// The eighth float pads the record to two vec4s, the one layout a storage
    /// array cannot read two ways.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_in(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], opacity: f32, unit: f32, z_step: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], 0.0, opacity, unit, z_step, 0.0]);
        self.draws.extend_from_slice(&[handle, vertex_cnt, index_cnt, uniform, pipeline]);
    }

    /// Words are exact in an f32 up to 2^24.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_instances(&mut self, pipeline: u32, handle: u32, group: u32, count: u32, lanes: [u32; 3], translation: [f32; 3], opacity: f32) {
        let [offset, stride, index] = lanes;
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms
            .extend_from_slice(&[translation[0], translation[1], translation[2], offset as f32, opacity, stride as f32, index as f32, 0.0]);
        self.draws.extend_from_slice(&[handle, group, count, uniform, pipeline]);
    }

    pub fn draw_count(&self) -> u32 {
        (self.draws.len() / DRAW_WORDS) as u32
    }

    /// Uploads, rewrites and frees start fresh; draws stay until the next frame.
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

/// A heap block the renderer mirrors, written whole each time and grown to the
/// next power of two so a small wobble in count does not reallocate.
#[derive(Default)]
pub struct Held {
    pub at: usize,
    pub bytes: u32,
    pub handle: u32,
}

impl Held {
    /// False when the heap refused; the block is then gone.
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
