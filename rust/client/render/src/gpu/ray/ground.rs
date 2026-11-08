// pxl-render/gpu/ray/ground.rs

//! The GPU's copy of the terrain bricks the core fills for the clip levels.
//! A brick is its words, a word of four palette indices for each cell, and
//! its ways' tree, which lives in a pool every brick shares. The runtime
//! reads each brick from the core's own memory when it runs the turn.

use pxl_engine::clipmap::{BRICK_CELLS, BRICK_WORDS, FRAME_BRICKS, SLOTS};
use pxl_engine::{way, Zoom};
use xpute_kit::collection::list::List;

use super::RaysPass;
use crate::frame::shader::floor;
use crate::gpu::api::{buffer_usage, bytes, release, room, write_buffer_bytes, Gpu};

/// What a frame hands of the terrain: `upload` offers the writer each brick
/// the core filled, and `pool` is how many child groups the ways' pool must
/// hold (`GroundBuffers::grow`).
pub struct GroundBricks<'a> {
    pub upload: &'a mut TerrainUpload,
    pub pool: u32,
}

/// A brick the core filled (pxl-core domain/layer/terrain/clipmap.rs
/// `upload`). Its words are laid out as pxl-engine `clipmap::BRICK_WORDS`
/// says. `tree` is its ways' tree in pieces, each its first child group in
/// the pool and the words from there. Every slice is the core's own memory:
/// the runtime reads it when it runs the turn, and the core asks
/// `in_flight` before it writes any of it again.
pub struct BrickWrite<'a> {
    pub slot: u32,
    pub cell_zoom: Zoom,
    pub brick: [i32; 2],
    pub words: &'a [u32],
    pub palette_indices: &'a [u32],
    pub tree: &'a [(u32, &'a [u32])],
}

/// Takes a brick and says whether it did. It refuses once the frame has no
/// room left, and the core offers the brick again the next frame.
pub type TerrainWrite<'a> = dyn FnMut(BrickWrite<'_>) -> bool + 'a;

pub type TerrainUpload = dyn for<'w> FnMut(&mut TerrainWrite<'w>);

/// The bytes `child_groups` child groups take in the ways' pool, which is
/// also where the next one starts. A child group is eight child pointers then
/// eight data words (pxl-engine `way::tree`).
const fn pool_bytes(child_groups: u32) -> u32 {
    child_groups * way::tree::CHILD_GROUP_WORDS * 4
}
const _: () = assert!(
    floor::fits(pool_bytes(way::tree::POOL_CHILD_GROUPS) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// A ways' pool with room for `child_groups`. `grow` copies it into the
/// larger pool that replaces it.
fn pool_buf(g: &Gpu, child_groups: u32) -> u32 {
    g.buf(
        pool_bytes(child_groups),
        buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
    )
}

/// The terrain's bricks on the GPU.
pub(super) struct GroundBuffers {
    /// Every slot's brick words (`GROUND_BYTES`).
    pub(super) ground: u32,
    /// Every slot's palette indices (`PALETTE_BYTES`).
    pub(super) palette_indices: u32,
    /// The ways' pool, and the child groups it has room for.
    pub(super) pool: u32,
    child_groups: u32,
    /// The bricks this frame handed, by cells' zoom and place. Decor's
    /// generation reads them as the bricks filled this frame
    /// (gpu/generator.rs `Generation::fresh`).
    pub(super) inheritance: List<(Zoom, [i32; 2]), { FRAME_BRICKS as usize }>,
}

impl GroundBuffers {
    /// Grows the ways' pool to hold `child_groups`, rounded up to a power of
    /// two and never past `way::tree::POOL_CHILD_GROUPS`, which holds any
    /// baked set. The old words are copied to the same offsets, so every child
    /// group a tree names keeps its words. `hand` grows the pool before it
    /// writes the frame's bricks, so those writes and the cast use the new
    /// buffer; the cast makes its groups again over the new id (`prepare`).
    ///
    /// A device out of memory may fail to make the larger buffer. Nothing
    /// reports it, as the runtime keeps no error scope, and the renderer goes
    /// on with a buffer the device holds as an error. The last doubling holds
    /// the old 32 MiB pool beside the new 64 MiB one until the copy is
    /// submitted.
    fn grow(&mut self, g: &Gpu, child_groups: u32) {
        let child_groups = child_groups
            .next_power_of_two()
            .min(way::tree::POOL_CHILD_GROUPS);
        if child_groups <= self.child_groups {
            return;
        }
        let pool = pool_buf(g, child_groups);
        let e = g.encoder();
        g.copy_buf(e, (self.pool, 0), (pool, 0), pool_bytes(self.child_groups));
        g.submit(e);
        release(self.pool);
        (self.pool, self.child_groups) = (pool, child_groups);
    }
}

const BRICK_BYTES: u32 = BRICK_WORDS * 4;

/// Every slot's brick in one storage buffer, which the cast's group binds
/// whole.
const GROUND_BYTES: u64 = SLOTS as u64 * BRICK_BYTES as u64;
const _: () = assert!(
    floor::fits(GROUND_BYTES, true),
    "GROUND_BYTES: the clip levels' bricks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// A brick's palette indices: a word for each cell, which packs its four.
const PALETTE_BRICK_BYTES: u32 = BRICK_CELLS * 4;

/// Every slot's palette indices in slot order, which the cast reads at a hit
/// (palette.wgsl). They take a buffer of their own, as the bricks' buffer
/// would bind past WebGPU's floor with them.
const PALETTE_BYTES: u64 = SLOTS as u64 * PALETTE_BRICK_BYTES as u64;
const _: () = assert!(
    floor::fits(PALETTE_BYTES, true),
    "PALETTE_BYTES: the bricks' palette indices bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

impl RaysPass {
    /// Uploads the bricks the core filled: at most `clipmap::FRAME_BRICKS` a
    /// frame, and only while the stream has room for each brick's records.
    /// The renderer calls it every frame, whether or not an eye casts, as the
    /// bricks are the terrain's and not an eye's. It runs before the frame's
    /// cast, which reads the clip levels' table the upload changed.
    pub fn hand(&mut self, g: &Gpu, ground: &mut GroundBricks) {
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let gb = self.ground.get_or_insert_with(|| {
            let child_groups = ground
                .pool
                .next_power_of_two()
                .min(way::tree::POOL_CHILD_GROUPS);
            GroundBuffers {
                ground: g.buf(GROUND_BYTES as u32, storage),
                palette_indices: g.buf(PALETTE_BYTES as u32, storage),
                pool: pool_buf(g, child_groups),
                child_groups,
                inheritance: List::new(),
            }
        });
        gb.grow(g, ground.pool);
        gb.inheritance.clear();
        (ground.upload)(&mut |q| {
            let records = 2 + q.tree.len();
            if gb.inheritance.len() == FRAME_BRICKS as usize
                || !room(records * write_buffer_bytes(&[]))
            {
                return false;
            }
            g.upload_buf(gb.ground, q.slot * BRICK_BYTES, bytes(q.words));
            g.upload_buf(
                gb.palette_indices,
                q.slot * PALETTE_BRICK_BYTES,
                bytes(q.palette_indices),
            );
            for &(first, words) in q.tree {
                g.upload_buf(gb.pool, pool_bytes(first), bytes(words));
            }
            gb.inheritance.push((q.cell_zoom, q.brick));
            true
        });
    }
}
