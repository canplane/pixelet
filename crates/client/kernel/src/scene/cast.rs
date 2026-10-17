// pxl-kernel/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/renderer.rs
//! `Cast`): the eye as the rays leave it and its lane's tangents, the eye's
//! windows as a table of the squares each holds, and each square the
//! windows want handed to the GPU once, off its tile's packet
//! (layer/terrain/window.rs `fill`).

use pxl_render::gpu::renderer::{Overwrites, TerrainWrite, CAST_EYE_WORDS};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;
use xpute_kit::collection::list::List;

use crate::kernel;
use crate::layer::object::preview;
use crate::layer::object::store::EntryAt;
use crate::layer::residency::Residency;
use crate::layer::terrain::tile::kept;
use crate::layer::terrain::window::{self, Handed};
use crate::layer::Base as _;
use crate::setting::schema_spec::RayDensity;
use crate::setting::store::settings_store;
use crate::view::frame::pose::view;
use crate::view::lens;

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

/// The eye's windows as the GPU's cast reads them (layer/terrain/window.rs
/// `table`).
pub fn window() -> &'static [i32] {
    window::table()
}

/// What is placed, for the GPU: the objects' tree's lanes as they live, the
/// room either has, and the store's revision, so the GPU takes them again
/// only when they changed.
pub fn overwrites() -> Overwrites<'static> {
    let (heap, data, room, revision) = kernel().objects.raw_lanes();
    Overwrites { heap, data, room, revision }
}

/// Each square the windows want to `write`, while it takes them: off the
/// packets kept for them, and off each void tile, held with none.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let terrain = TerrainTile::residency();
    kept().send(pxl_render::gpu::api::device_generation(), &mut |frame, pkt| {
        let Some(t) = terrain.in_frame(frame) else { return true };
        let Some(held) = terrain.get(t) else { return true };
        window::fill(t, Handed::Packet(pkt, held), write)
    });
    // Taken first, as filling reads the residency. A tile held with no frame
    // is held only while asked, and no more are asked than its frames.
    let mut void = List::<Tile, { Residency::<TerrainTile>::FRAMES as usize }>::new();
    for (p, _) in terrain.held().filter(|&(p, _)| terrain.frame(p).is_none()) {
        void.push(p);
    }
    for p in void {
        if !window::fill(p, Handed::Void, write) {
            return;
        }
    }
}
