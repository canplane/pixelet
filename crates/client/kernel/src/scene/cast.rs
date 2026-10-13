// pxl-kernel/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/renderer.rs
//! `Cast`): the eye as the rays leave it and its lane's tangents, the
//! terrain's held tiles as a window
//! a tile zoom, and each tile handed to the GPU once, from where it lives in
//! its residency frame, which demand frees only by eviction before the stage
//! writes, so the range stays unwritten until the page replays the turn.

use pxl_render::gpu::api::bytes;
use pxl_render::gpu::renderer::{TerrainWrite, CAST_EYE_WORDS};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::demand::REACH_ACROSS;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};
use xpute_guest::global::Global;

use crate::layer::residency::Residency;
use crate::layer::Base as _;
use crate::setting::schema_spec::RayDensity;
use crate::setting::store::settings_store;
use crate::view::frame::pose::view;
use crate::view::frame::resolution::world_ratio;
use crate::view::sight;

/// The terrain frames the GPU keeps a tile's lanes for.
pub const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// Per frame, the tile whose lanes the GPU holds there.
static ON_GPU: Global<Vec<Option<Tile>>> = Global::new();

fn on_gpu() -> &'static mut Vec<Option<Tile>> {
    ON_GPU.get(|| vec![None; FRAMES as usize])
}

/// The shards the service area serves, as the GPU reads them.
static SHARDS: Global<Vec<[i32; 4]>> = Global::new();

/// Each shard the service area serves: its tile zoom, x (as `wrap_index`
/// names it) and y, and a spare; one serving nothing (zoom 255) where none
/// is declared, since a binding holds at least one.
pub fn shards() -> &'static [[i32; 4]] {
    let s = SHARDS.get(Vec::new);
    s.clear();
    crate::io::source::served(&mut |zoom, [x, y]| s.push([zoom as i32, x, y, 0]));
    if s.is_empty() {
        s.push([255, 0, 0, 0]);
    }
    s
}

/// The window's words, kept so a frame takes nothing from the heap.
static WINDOW: Global<Vec<i32>> = Global::new();

/// The tangents of the lane's columns then rows (`sight::gpu_cast`), kept
/// until the next frame's cast rewrites them, after the page has replayed
/// this turn.
static TANGENTS: Global<Vec<f32>> = Global::new();

/// The eye as the GPU's rays leave it, over a lane of the cells the `rays`
/// setting asks, and the lane's column and row tangents: the CPU's cast's
/// one rule (`sight::gpu_cast`). None where the CPU casts, or without a
/// screen.
pub fn eye(o: &Observation) -> Option<([u32; CAST_EYE_WORDS], &'static [f32])> {
    let tau = sight::tau();
    let v = view();
    let px = [v.width, v.height].map(|n| (n * world_ratio()).ceil().max(1.0) as u32);
    let across = match settings_store().rays {
        RayDensity::cpu => return None,
        RayDensity::cells => sight::grid_across(o, tau)?,
        RayDensity::half => px.map(|n| n.div_ceil(2)),
        RayDensity::full => px,
    };
    let (mut eye, tangents) = ([0u32; CAST_EYE_WORDS], TANGENTS.get(Vec::new));
    sight::gpu_cast(o, tau, across, &mut eye, tangents)?;
    Some((eye, tangents))
}

/// Per tile zoom from the top to the leaves, the tiles about the eye the GPU
/// holds: the window's south-west tile, `REACH_ACROSS` and a spare, then
/// `REACH_ACROSS`² words row by row, each its frame plus one, zero where the
/// GPU holds none.
pub fn window(o: &Observation) -> &'static [i32] {
    let w = WINDOW.get(Vec::new);
    w.clear();
    let terrain = TerrainTile::residency();
    let half = i64::from(REACH_ACROSS / 2);
    for zoom in GROUND.top()..=LEAF_ZOOM {
        let at = [0, 1].map(|k| (o.eye[k] / units(zoom)).floor() as i64 - half);
        w.extend_from_slice(&[at[0] as i32, at[1] as i32, REACH_ACROSS as i32, 0]);
        for dy in 0..i64::from(REACH_ACROSS) {
            for dx in 0..i64::from(REACH_ACROSS) {
                let p = tile_at(zoom, [at[0] + dx, at[1] + dy]);
                let held = terrain.frame(p).filter(|&f| on_gpu()[f as usize] == Some(p));
                w.push(held.map_or(0, |f| f as i32 + 1));
            }
        }
        // A zoom's words fill whole rows of four.
        w.resize(w.len().next_multiple_of(4), 0);
    }
    w
}

/// Each held tile the GPU does not yet hold in its frame to `write`: its
/// frame, ground looks, padded heights, and its index's extents and their
/// coarse levels.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let terrain = TerrainTile::residency();
    for (p, tile) in terrain.held() {
        let Some(frame) = terrain.frame(p) else { continue };
        let slot = &mut on_gpu()[frame as usize];
        if *slot == Some(p) {
            continue;
        }
        write(frame, &tile.ground.cells, bytes(&tile.ele), bytes(tile.index.extent()), bytes(tile.index.coarse()));
        *slot = Some(p);
    }
}
