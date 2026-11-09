// pxl-render/ray/ground.rs

//! The GPU's copy of the terrain bricks the core fills for the clip levels.
//! A brick is its words, a word of four palette indices for each cell, and
//! its kept spans, a run in a pool every brick shares. The runtime reads each
//! brick from the core's own memory when it runs the turn.

use pxl_engine::clipmap::{BRICK_WORDS, FRAME_BRICKS, SLOTS, WAYS_POOL_WORDS};
use pxl_spec::brick::BRICK_CELLS;
use pxl_spec::space::coord::zoom::Zoom;
use xpute_kit::collection::list::List;

use super::RayPass;
use crate::frame::shader::floor;
use crate::gpu::api::{buffer_usage, bytes, release, room, write_buffer_bytes, Gpu};

/// What a frame hands of the terrain: `upload` offers the writer each brick
/// the core filled, and `pool` is how many words the ways' pool must hold
/// (`GroundBuffer::grow`).
pub struct GroundInput<'a> {
    pub upload: &'a mut TerrainUpload,
    pub pool: u32,
}

/// A brick the core filled (pxl-core domain/layer/terrain/clipmap.rs
/// `upload`). Its words are laid out as pxl-engine `clipmap::BRICK_WORDS`
/// says. `run` is its run of kept spans in the ways' pool, in pieces, each its
/// first word in the pool and the words from there, none where it keeps no span.
/// Every slice is the core's own memory: the runtime reads it when it runs the turn, and the core asks
/// `in_flight` before it writes any of it again.
pub struct BrickWrite<'a> {
    pub slot: u32,
    pub cell_zoom: Zoom,
    pub brick: [i32; 2],
    pub words: &'a [u32],
    pub palette_idxs: &'a [u32],
    pub run: &'a [(u32, &'a [u32])],
}

/// Takes a brick and says whether it did. It refuses once the frame has no
/// room left, and the core offers the brick again the next frame.
pub type TerrainWrite<'a> = dyn FnMut(BrickWrite<'_>) -> bool + 'a;

pub type TerrainUpload = dyn for<'w> FnMut(&mut TerrainWrite<'w>);

/// The bytes `words` words take in the ways' pool, which is also where the
/// next one starts (pxl-core domain/layer/terrain/pool.rs).
const fn pool_bytes(words: u32) -> u32 {
    words * 4
}
const _: () = assert!(
    floor::fits(pool_bytes(WAYS_POOL_WORDS) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// A ways' pool with room for `words`. `grow` copies it into the larger
/// buffer that replaces it.
fn pool_buf(g: &Gpu, words: u32) -> u32 {
    g.buf(
        pool_bytes(words),
        buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
    )
}

/// The terrain's bricks on the GPU.
pub(super) struct GroundBuffer {
    /// Every slot's brick words (`GROUND_BYTES`).
    pub(super) ground: u32,
    /// Every slot's palette indices (`PALETTE_BYTES`).
    pub(super) palette_idxs: u32,
    /// The ways' pool, and the words it has room for.
    pub(super) pool: u32,
    word_cnt: u32,
    /// The bricks this frame handed, by cells' zoom and place. Decor's
    /// generation reads them as the bricks filled this frame
    /// (gpu/generator.rs `Generation::fresh`).
    pub(super) bricks: List<(Zoom, [i32; 2]), { FRAME_BRICKS as usize }>,
}

impl GroundBuffer {
    /// Grows the ways' pool to hold `words`, rounded up to a power of two
    /// and never past `WAYS_POOL_WORDS`. The old words are copied to the same
    /// offsets, so every run a brick names keeps its words; only bricks' runs
    /// are ever written here. `hand` grows the pool before it writes the
    /// frame's bricks, so those writes and the cast use the new buffer; the
    /// cast makes its groups again over the new id (`prepare`).
    ///
    /// A device out of memory may fail to make the larger buffer. Nothing
    /// reports it, as the runtime keeps no error scope, and the renderer goes
    /// on with a buffer the device holds as an error. The last doubling holds
    /// the old 32 MiB pool beside the new 64 MiB one until the copy is
    /// submitted.
    fn grow(&mut self, g: &Gpu, words: u32) {
        let words = words.next_power_of_two().min(WAYS_POOL_WORDS);
        if words <= self.word_cnt {
            return;
        }
        let pool = pool_buf(g, words);
        let e = g.encoder();
        g.copy_buf(e, (self.pool, 0), (pool, 0), pool_bytes(self.word_cnt));
        g.submit(e);
        release(self.pool);
        (self.pool, self.word_cnt) = (pool, words);
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

impl RayPass {
    /// Uploads the bricks the core filled: at most `clipmap::FRAME_BRICKS` a
    /// frame, and only while the stream has room for each brick's records.
    /// The renderer calls it every frame, whether or not an eye casts, as the
    /// bricks are the terrain's and not an eye's. It runs before the frame's
    /// cast, which reads the clip levels' table the upload changed.
    pub fn hand(&mut self, g: &Gpu, ground: &mut GroundInput) {
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let gb = self.ground.get_or_insert_with(|| {
            let word_cnt = ground.pool.next_power_of_two().min(WAYS_POOL_WORDS);
            GroundBuffer {
                ground: g.buf(GROUND_BYTES as u32, storage),
                palette_idxs: g.buf(PALETTE_BYTES as u32, storage),
                pool: pool_buf(g, word_cnt),
                word_cnt,
                bricks: List::new(),
            }
        });
        gb.grow(g, ground.pool);
        gb.bricks.clear();
        (ground.upload)(&mut |q| {
            let records = 2 + q.run.len();
            if gb.bricks.len() == FRAME_BRICKS as usize || !room(records * write_buffer_bytes(&[]))
            {
                return false;
            }
            g.upload_buf(gb.ground, q.slot * BRICK_BYTES, bytes(q.words));
            g.upload_buf(
                gb.palette_idxs,
                q.slot * PALETTE_BRICK_BYTES,
                bytes(q.palette_idxs),
            );
            for &(first, words) in q.run {
                g.upload_buf(gb.pool, pool_bytes(first), bytes(words));
            }
            gb.bricks.push((q.cell_zoom, q.brick));
            true
        });
    }
}
