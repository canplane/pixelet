// gpu-query-shader/harness.rs

//! The harness's entries (gpu-query/runner): the caves per node, a node's
//! seed, and a tile's ground per cell, each diffed against the CPU.

use pxl_portable::caves::{self, Cave};
use pxl_portable::ground::{self, Tile, TILE_CELLS};
use pxl_portable::LEAF_CELL;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

/// The nodes asked: `zoom`, and the first node's index on each axis.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ask {
    pub zoom: u32,
    pub first: [i32; 3],
    pub across: [u32; 3],
}

/// Node `id` of `ask`.
pub fn node(ask: &Ask, id: [u32; 3]) -> [i32; 3] {
    [ask.first[0] + id[0] as i32, ask.first[1] + id[1] as i32, ask.first[2] + id[2] as i32]
}

/// Node `id` of `ask` answered: bit 0 hollow, bit 1 whole known, bit 2 whole's value.
pub fn answer(ask: &Ask, id: [u32; 3]) -> u32 {
    let cave = Cave::of(ask.zoom, node(ask, id));
    let (known, value) = cave.whole(ask.zoom);
    u32::from(cave.hollow()) | u32::from(known) << 1 | u32::from(value) << 2
}

fn index(ask: &Ask, id: UVec3) -> Option<usize> {
    (id.x < ask.across[0] && id.y < ask.across[1] && id.z < ask.across[2]).then(|| (id.x + ask.across[0] * (id.y + ask.across[1] * id.z)) as usize)
}

#[spirv(compute(threads(4, 4, 4)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    if let Some(i) = index(ask, id) {
        out[i] = answer(ask, [id.x, id.y, id.z]);
    }
}

#[spirv(compute(threads(4, 4, 4)))]
pub fn seed_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    if let Some(i) = index(ask, id) {
        out[i] = caves::node_seed(0xca7e_0001, (ask.zoom, node(ask, [id.x, id.y, id.z])));
    }
}

/// One tile's frame in the ground buffers: 4 looks a word and 2 heights a
/// word, low first.
pub struct Frame<'a> {
    pub looks: &'a [u32],
    pub heights: &'a [u32],
}

impl Tile for Frame<'_> {
    fn look(&self, x: u32, y: u32) -> u32 {
        let i = (y * TILE_CELLS + x) as usize;
        self.looks[i / 4] >> (8 * (i % 4)) & 0xff
    }
    fn height(&self, x: u32, y: u32) -> i32 {
        // The half's sign carried up: shifted to the top, then back.
        ground::height(|i| ((self.heights[i / 2] >> (16 * (i % 2))) << 16) as i32 >> 16, [x, y])
    }
}

/// Cell `id` of the tile at the leaves: its look in bits 0..8, its block
/// at `ask.first[2]` without caves in bits 8..16, its height in the high half.
pub fn ground_of(ask: &Ask, tile: &Frame, id: [u32; 2]) -> u32 {
    let (look, h) = ground::read(tile, [id[0] as i32, id[1] as i32], 1);
    let cell = ground::Cell {
        look,
        top: ground::blocks(LEAF_CELL, h as f32),
        fold: [look; 4],
    };
    let (block, _) = cell.block(LEAF_CELL, ask.first[2], false);
    look | block << 8 | (h as u32 & 0xffff) << 16
}

#[spirv(compute(threads(8, 8, 1)))]
pub fn ground_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] heights: &[u32],
) {
    if let Some(i) = index(ask, id) {
        out[i] = ground_of(ask, &Frame { looks, heights }, [id.x, id.y]);
    }
}
