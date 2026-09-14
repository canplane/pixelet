//! What the renderer draws, as lists the glue executes against WebGPU: the
//! blocks to put on the GPU and the ones to let go, and each frame the
//! draws, with what each needs beyond its geometry. The glue holds no
//! decision — which mesh, at what weight, where — only the API calls; a
//! native shell executes the same lists against wgpu.
//!
//! A block is named by a handle the core hands out when a tier lands; the
//! upload says where the block lies in the memory and how long it is, and
//! the glue reads the lanes by the mesher's layout from the vertex count.
//! Uploads and frees accumulate until the glue takes them, whatever calls
//! made them (a tick, a build now); the draws are the last tick's. A free
//! cancels an upload not yet taken: the block is the glue's to release the
//! moment it is told, and may be someone else's by the time the list runs.

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

#[derive(Default)]
pub struct DrawList {
    uploads: Vec<u32>,
    frees: Vec<u32>,
    draws: Vec<u32>,
    uniforms: Vec<f32>,
    next_handle: u32,
    /// The lanes the glue last took, kept until it takes again.
    taken: Taken,
}

#[derive(Default)]
struct Taken {
    uploads: Vec<u32>,
    frees: Vec<u32>,
    draws: Vec<u32>,
    uniforms: Vec<f32>,
}

impl DrawList {
    /// A handle for a block the glue is to upload.
    pub fn upload(&mut self, at: u32, bytes: u32, vertex_cnt: u32, index_cnt: u32) -> u32 {
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let handle = self.next_handle;
        self.uploads.extend_from_slice(&[handle, at, bytes, vertex_cnt, index_cnt]);
        handle
    }

    /// The block behind `handle` is gone from the glue's memory, or about to
    /// be; 0 names no block.
    pub fn free(&mut self, handle: u32) {
        if handle == 0 {
            return;
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

    /// Starts this frame's draws over.
    pub fn begin_frame(&mut self) {
        self.draws.clear();
        self.uniforms.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, pipeline: u32, handle: u32, vertex_cnt: u32, index_cnt: u32, translation: [f32; 3], visible: f32, opacity: f32) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], visible, opacity, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, vertex_cnt, index_cnt, uniform, pipeline]);
    }

    /// A decor group's instances: `offset` is the group's first lane word in
    /// the block (decor.rs's layout), exact in a float to 2^24 words.
    pub fn draw_decor(&mut self, handle: u32, group: u32, count: u32, offset: u32, translation: [f32; 3]) {
        let uniform = (self.uniforms.len() / UNIFORM_FLOATS) as u32;
        self.uniforms.extend_from_slice(&[translation[0], translation[1], translation[2], offset as f32, 1.0, 0.0, 0.0, 0.0]);
        self.draws.extend_from_slice(&[handle, group, count, uniform, PIPELINE_DECOR]);
    }

    /// Hands the lists to the glue: the lanes' addresses and counts, valid
    /// until the next take. Uploads and frees start fresh; the draws stay
    /// until the next frame writes them.
    pub fn take(&mut self) -> [usize; 8] {
        core::mem::swap(&mut self.taken.uploads, &mut self.uploads);
        core::mem::swap(&mut self.taken.frees, &mut self.frees);
        self.uploads.clear();
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
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_cancels_an_upload_not_yet_taken_and_survives_one_taken() {
        let mut d = DrawList::default();
        let a = d.upload(100, 40, 2, 3);
        let b = d.upload(200, 40, 2, 3);
        d.free(a);
        let lanes = d.take();
        assert_eq!((lanes[1], lanes[3]), (1, 0), "a cancelled, b taken, no free");
        d.free(b);
        let lanes = d.take();
        assert_eq!((lanes[1], lanes[3]), (0, 1));
        d.begin_frame();
        d.draw(PIPELINE_CHUNK, b, 2, 3, [1.0, 2.0, 3.0], 1.0, 1.0);
        let lanes = d.take();
        assert_eq!((lanes[5], lanes[7]), (1, 1));
    }
}
