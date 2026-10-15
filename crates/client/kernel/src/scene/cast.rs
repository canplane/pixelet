// pxl-kernel/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/renderer.rs
//! `Cast`): the eye as the rays leave it and its lane's tangents, the
//! terrain's held tiles as a window
//! a tile zoom, and each tile handed to the GPU once: its four looks copied
//! off its packet, its heights from where they live in its residency frame,
//! which demand frees only by eviction before the stage writes, so the range
//! stays unwritten until the page replays the turn.

use pxl_render::gpu::api::bytes;
use pxl_render::gpu::renderer::{Overwrites, TerrainWrite, CAST_EYE_WORDS};
use pxl_spec::bake::tile::terrain::{handed, GroundId, TerrainTile, LOOK_LANE};
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::demand::REACH_ACROSS;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, LEAF_ZOOM};
use xpute_guest::global::Global;

use crate::kernel;
use crate::layer::object::preview;
use crate::layer::object::store::EntryAt;
use crate::layer::residency::floor;
use crate::layer::residency::Residency;
use crate::layer::terrain::tile::{asked, cells_back, kept};
use crate::layer::Base as _;
use crate::setting::schema_spec::RayDensity;
use crate::setting::store::settings_store;
use crate::view::frame::pose::view;
use crate::view::lens;
use crate::view::observation::reach;

/// The terrain frames the GPU keeps a tile's lanes for.
pub const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// A leaf's looks expanded, four a cell, kept so a frame takes nothing from the heap.
static LOOKS: Global<Vec<GroundId>> = Global::new();

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

/// The tangents of the lane's columns then rows (`lens::gpu_cast`), kept
/// until the next frame's cast rewrites them, after the page has replayed
/// this turn.
static TANGENTS: Global<Vec<f32>> = Global::new();

/// The eye as the GPU's rays leave it, over a lane of the cells the `rays`
/// setting asks, and the lane's column and row tangents (`lens::gpu_cast`),
/// then the pointer's preview as the world answers through it (query/world.rs
/// `previewed`), its cut to air and its left-out leaf, each X, Y, Z and the
/// zoom, the zoom `u32::MAX` for none. None without a screen.
pub fn eye(o: &Observation) -> Option<([u32; CAST_EYE_WORDS], &'static [f32])> {
    let tau = lens::tau();
    let v = view();
    // The screen's CSS pixels, whatever the device draws them at: the screen
    // pass spreads a cell over the pixels it covers.
    let px = [v.width, v.height].map(|n| n.ceil().max(1.0) as u32);
    let across = match settings_store().rays {
        RayDensity::cells => lens::grid_across(o, tau)?,
        RayDensity::half => px.map(|n| n.div_ceil(2)),
        RayDensity::full => px,
    };
    let (mut eye, tangents) = ([0u32; CAST_EYE_WORDS], TANGENTS.get(Vec::new));
    lens::gpu_cast(o, tau, across, &mut eye, tangents)?;
    let edits = preview::current();
    let cut = |e: Option<EntryAt>| e.map_or([0, 0, 0, u32::MAX], |e| [e.at()[0] as u32, e.at()[1] as u32, e.at()[2] as u32, e.zoom()]);
    eye[lens::EYE_WORDS..lens::EYE_WORDS + 4].copy_from_slice(&cut(edits.carve));
    eye[lens::EYE_WORDS + 4..].copy_from_slice(&cut(edits.hide));
    Some((eye, tangents))
}

/// Per tile zoom from the top to the leaves, the tiles about the eye the GPU
/// holds: the window's south-west tile, `REACH_ACROSS` and a spare, then
/// `REACH_ACROSS`² words row by row, each its frame plus one, zero where the
/// GPU holds none. Its squares are those the view asks for there, within
/// view/observation.rs `reach` of the eye as `squares_within` counts them,
/// so the GPU reads the tiles the CPU does.
pub fn window(o: &Observation) -> &'static [i32] {
    let w = WINDOW.get(Vec::new);
    w.clear();
    let terrain = TerrainTile::residency();
    let top = floor::top().map(|t| t.max(GROUND.top()));
    for zoom in GROUND.top()..=LEAF_ZOOM {
        let r = top.and_then(|top| reach(o, lens::tau(), GROUND, top, zoom)).unwrap_or(0.0);
        let at = [0, 1].map(|k| ((o.eye[k] - r) / units(zoom)).floor() as i64);
        w.extend_from_slice(&[at[0] as i32, at[1] as i32, REACH_ACROSS as i32, 0]);
        for dy in 0..i64::from(REACH_ACROSS) {
            for dx in 0..i64::from(REACH_ACROSS) {
                let p = tile_at(zoom, [at[0] + dx, at[1] + dy]);
                let held = terrain.frame(p);
                w.push(held.map_or(0, |f| f as i32 + 1));
            }
        }
        // A zoom's words fill whole rows of four.
        w.resize(w.len().next_multiple_of(4), 0);
    }
    w
}

/// What is placed, for the GPU: the objects' tree's lanes as they live, the
/// room either has, and the store's revision, so the GPU takes them again
/// only when they changed.
pub fn overwrites() -> Overwrites<'static> {
    let (heap, data, room, revision) = kernel().objects.raw_lanes();
    Overwrites { heap, data, room, revision }
}

/// Each tile the GPU does not yet hold in its frame to `write`, while it
/// takes them: its frame, its four looks a cell and its samples, and where
/// its cells come back to.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let room = LOOKS.get(|| vec![0; LOOK_LANE]);
    // A packet `receive` read reads again.
    kept().send(&mut |frame, pkt| {
        let Ok((looks, ele)) = handed(pkt, room) else { return true };
        let back = cells_back(frame);
        let taken = write(frame, looks, bytes(ele), back);
        if taken {
            asked(frame, back.ticket);
        }
        taken
    });
}
