// pxl-render/gpu/ray/ground.rs

//! The clip levels' bricks on the GPU, a copy of what the core fills: each
//! brick's words, its four palette indices a cell and its ways' tree, read
//! where they lie as the frame hands them (`hand`).

use pxl_engine::clipmap::{BRICK_CELLS, BRICK_WORDS, FRAME_BRICKS, SLOTS};
use pxl_engine::{way, Zoom};
use xpute_kit::collection::list::List;

use super::RaysPass;
use crate::frame::shader::floor;
use crate::gpu::api::{buffer_usage, bytes, release, room, write_buffer_bytes, Gpu};

/// The ground's clip levels as the GPU holds them (pxl-engine `clipmap`, their
/// bricks in `SLOTS` slots): the bricks to hand it, and the child groups its
/// pool must hold (`GroundBuffers::grow`).
pub struct GroundBricks<'a> {
    /// Hands each brick the core filled to the writer.
    pub upload: &'a mut TerrainUpload,
    pub pool: u32,
}

/// A brick handed to the GPU, the record the ground's clip levels hand on
/// (pxl-core domain/layer/terrain/clipmap.rs `upload`): its slot, its cells'
/// zoom and the brick; its words (pxl-engine `clipmap::BRICK_WORDS`), its four
/// palette indices a cell, and its ways' tree, a piece each its first child
/// group and its words. They lie as the core holds them, its owner asking
/// `in_flight` before it writes them again.
pub struct BrickWrite<'a> {
    pub slot: u32,
    pub cell: Zoom,
    pub brick: [i32; 2],
    pub words: &'a [u32],
    pub palette_indices: &'a [u32],
    pub tree: &'a [(u32, &'a [u32])],
}

/// Takes a brick while the frame has room for it.
pub type TerrainWrite<'a> = dyn FnMut(BrickWrite<'_>) -> bool + 'a;

pub type TerrainUpload = dyn for<'w> FnMut(&mut TerrainWrite<'w>);

/// A child group's first word in the ways' pool, in bytes: the child groups whole,
/// its eight child pointers then its eight data words (pxl-engine `way::tree`).
const fn pool_bytes(child_groups: u32) -> u32 {
    child_groups * way::tree::CHILD_GROUP_WORDS * 4
}
const _: () = assert!(
    floor::fits(pool_bytes(way::tree::POOL_CHILD_GROUPS) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The ways' pool of `child_groups`, copied from as it grows, and into.
fn pool_buf(g: &Gpu, child_groups: u32) -> u32 {
    g.buf(
        pool_bytes(child_groups),
        buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
    )
}

/// The terrain's bricks on the GPU.
pub(super) struct GroundBuffers {
    /// Per slot of the clip levels' bricks (`clipmap::SLOTS`), its columns,
    /// levels, cells as laid and root (pxl-engine `clipmap::BRICK_WORDS`).
    pub(super) ground: u32,
    /// Per slot, its cells' four palette indices (`PALETTE_BYTES`).
    pub(super) palette_indices: u32,
    /// The ways' pool, which the cast reads, and the child groups it holds.
    pub(super) pool: u32,
    child_groups: u32,
    /// The bricks the frame handed, `clipmap::FRAME_BRICKS` at most: each its
    /// cells' zoom and the brick.
    pub(super) handed: List<(Zoom, [i32; 2]), { FRAME_BRICKS as usize }>,
}

impl GroundBuffers {
    /// The ways' pool holding `child_groups`, doubled as far as it takes, never
    /// past `way::tree::POOL_CHILD_GROUPS`, the bake's bound, at which no
    /// brick of a baked set is refused: what it held copied where it was, so
    /// every id a tree names holds what it held. Ahead of the frame's bricks, so
    /// their writes and the cast read the new one; the cast's groups follow its
    /// id (`prepare`).
    ///
    /// A known gap: a device out of memory may fail to make the larger buffer,
    /// and nothing hears it, the runtime keeping no error scope, so the
    /// renderer reads on with a buffer the device holds as an error. The
    /// last doubling holds the old pool beside the new, 32 MiB with 64,
    /// until the copy is submitted.
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

/// A brick on the GPU, in bytes.
const BRICK_BYTES: u32 = BRICK_WORDS * 4;

/// The clip levels' bricks on the GPU, in bytes: one storage buffer, bound
/// whole by the cast's group.
const GROUND_BYTES: u64 = SLOTS as u64 * BRICK_BYTES as u64;
const _: () = assert!(
    floor::fits(GROUND_BYTES, true),
    "GROUND_BYTES: the clip levels' bricks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// A brick's four palette indices a cell on the GPU, in bytes.
const PALETTE_BRICK_BYTES: u32 = BRICK_CELLS * 4;

/// The clip levels' bricks' cells' four palette indices, a word a cell in the
/// slots' order, read by the cast at a hit (palette.wgsl). Their own buffer,
/// as the bricks' binding has no room for them.
const PALETTE_BYTES: u64 = SLOTS as u64 * PALETTE_BRICK_BYTES as u64;
const _: () = assert!(
    floor::fits(PALETTE_BYTES, true),
    "PALETTE_BYTES: the bricks' palette indices bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

impl RaysPass {
    /// The bricks the core filled written into the GPU's, each a record however
    /// large, read where it lies; whether or not an eye casts, as a tile's
    /// cells are no eye's. Before the frame's cast, which reads the table
    /// their handing moved.
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
                handed: List::new(),
            }
        });
        gb.grow(g, ground.pool);
        gb.handed.clear();
        (ground.upload)(&mut |q| {
            let records = 2 + q.tree.len();
            if gb.handed.len() == FRAME_BRICKS as usize || !room(records * write_buffer_bytes(&[]))
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
            gb.handed.push((q.cell, q.brick));
            true
        });
    }
}
