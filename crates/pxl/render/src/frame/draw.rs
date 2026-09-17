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
use xpute_core::sys::mem::{alloc, free};

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

/// The pipelines a draw names. A chunk draw's words are (handle, vertex
/// count, index count, uniform, pipeline); a decor draw's are (the decor
/// block's handle, the group — which kind and variant, in teaching order —,
/// the instance count, uniform, pipeline), its uniform carrying the group's
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
/// An actor (crates/pxl/kernel/src/io/load.rs): words (the model's handle, the joint palette's
/// handle, the model id, uniform, pipeline), its uniform's fourth float the
/// turn about Z.
pub const PIPELINE_SKINNED: u32 = 5;
/// The location puck (crates/pxl/kernel/src/layer/marker/location_puck.rs): the zone's instances as (the zone block's
/// handle, the instance count, 0, uniform, pipeline), translated to the
/// zone's centre; the beacon as (0, night, 0, uniform, pipeline), its
/// uniform's fourth float its spin.
pub const PIPELINE_PUCK_ZONE: u32 = 6;
pub const PIPELINE_PUCK_BEACON: u32 = 7;
/// The badges (crates/pxl/kernel/src/layer/billboard/field.rs): (the block's handle, the count, 0, uniform,
/// pipeline), translated to the placement's origin.
pub const PIPELINE_BADGE: u32 = 8;
/// Line segments over everything: handle, vertex count.
pub const PIPELINE_DEBUG: u32 = 9;
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
    /// The lanes the shell last took, kept until it takes again.
    taken: Taken,
}

#[derive(Default)]
struct Taken {
    uploads: Vec<u32>,
    rewrites: Vec<u32>,
    frees: Vec<u32>,
    draws: Vec<u32>,
    uniforms: Vec<f32>,
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
        if handle == 0 || self.rewrites.chunks_exact(REWRITE_WORDS).any(|r| r[0] == handle) {
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

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], visible: f32, opacity: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], visible, opacity, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, vertex_cnt, index_cnt, uniform, pipeline]);
    }

    /// A decor group's instances: `offset` is the group's first lane word in
    /// the block (crates/pxl/kernel/src/layer/decor/build.rs's layout), exact in a float to 2^24 words.
    pub fn draw_decor(&mut self, handle: u32, group: u32, count: u32, offset: u32, translation: [f32; 3]) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], offset as f32, 1.0, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, group, count, uniform, PIPELINE_DECOR]);
    }

    /// The draws the last frame listed.
    pub fn draw_count(&self) -> u32 {
        (self.draws.len() / DRAW_WORDS) as u32
    }

    /// Hands the lists to the shell: the lanes' addresses and counts, valid
    /// until the next take. Uploads and frees start fresh; the draws stay
    /// until the next frame writes them.
    pub fn take(&mut self) -> [usize; 10] {
        core::mem::swap(&mut self.taken.uploads, &mut self.uploads);
        core::mem::swap(&mut self.taken.rewrites, &mut self.rewrites);
        core::mem::swap(&mut self.taken.frees, &mut self.frees);
        self.uploads.clear();
        self.rewrites.clear();
        self.frees.clear();
        self.taken.draws.clear();
        self.taken.draws.extend_from_slice(&self.draws);
        self.taken.uniforms.clear();
        self.taken.uniforms.extend_from_slice(&self.uniforms);
        let t = &self.taken;
        [
            t.uploads.as_ptr() as usize,
            t.uploads.len() / UPLOAD_WORDS,
            t.frees.as_ptr() as usize,
            t.frees.len(),
            t.draws.as_ptr() as usize,
            t.draws.len() / DRAW_WORDS,
            t.uniforms.as_ptr() as usize,
            t.uniforms.len() / UNIFORM_FLOATS,
            t.rewrites.as_ptr() as usize,
            t.rewrites.len() / REWRITE_WORDS,
        ]
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

#[cfg(test)]
#[path = "draw.test.rs"]
mod test;
