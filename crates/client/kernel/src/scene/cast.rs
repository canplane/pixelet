// pxl-kernel/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/renderer.rs
//! `Cast`): the eye as the rays leave it, a ray a drawn pixel, the eye's
//! windows as a table of the squares each holds, and each square the
//! windows want handed to the GPU once, off its tile's packet
//! (layer/terrain/window.rs `fill`).

use pxl_render::gpu::renderer::{Overwrites, TerrainWrite};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec_core::eye;
use xpute_guest::global::Global;

use crate::layer::kept::Offered;
use crate::layer::object::preview;
use crate::layer::object::store::objects;
use crate::layer::terrain::tile::kept;
use crate::layer::terrain::window::{self, Handed};
use crate::layer::Base as _;
use crate::setting::store::settings_store;
use crate::view::lens;
use pxl_spec::bake::constant::EntryAt;

/// The service area's index as the GPU reads it, and whether a declared
/// source made it: one is declared once (io/source.rs `declare`), so it is
/// made then and not again.
static SHARDS: Global<(bool, Vec<[i32; 4]>)> = Global::new();

/// The area (pxl-spec-core `area::Area`): every cell's nine by its key
/// (`area::shard_key`), four a row; a row of none where none is declared,
/// since a binding holds at least one, and a read past it is none.
pub fn shards() -> &'static [[i32; 4]] {
    let declared = crate::io::source::top().is_some();
    let (made, s) = SHARDS.get(|| (false, Vec::new()));
    if s.is_empty() || *made != declared {
        s.clear();
        let nines = crate::io::source::nines();
        let nine = |i: usize| nines.get(i).copied().map_or(0, i32::from);
        s.extend((0..nines.len().div_ceil(4).max(1)).map(|r| core::array::from_fn(|k| nine(4 * r + k))));
        *made = declared;
    }
    s
}

/// The eye as the GPU's rays leave it, a ray a drawn pixel (`lens::gpu_cast`),
/// then the pointer's preview as the world answers through it (query/world.rs
/// `previewing`), its cut to air and its left-out leaf, each X, Y, Z and the
/// zoom (pxl-spec-core `eye`). None without a screen.
pub fn eye(o: &Observation) -> Option<[u32; eye::WORDS]> {
    let mut words = [0u32; eye::WORDS];
    lens::gpu_cast(o, lens::tau(), settings_store().ray_steps, &mut words)?;
    let edits = preview::current();
    let cut = |e: Option<EntryAt>| e.map_or([0, 0, 0, eye::NO_CUT], |e| [e.at()[0] as u32, e.at()[1] as u32, e.at()[2] as u32, e.zoom()]);
    words[eye::CARVE..eye::CARVE + eye::CUT_WORDS].copy_from_slice(&cut(edits.carve));
    words[eye::HIDE..eye::HIDE + eye::CUT_WORDS].copy_from_slice(&cut(edits.hide));
    Some(words)
}

/// What is placed, for the GPU: the objects' tree's lanes as they live, the
/// room either has, the store's revision, and the blocks written since the
/// revision the lanes were last handed at, so the GPU takes only those
/// again where it holds that one (layer/object/store.rs `gpu_lanes`).
pub fn overwrites() -> Overwrites<'static> {
    let l = objects().gpu_lanes();
    Overwrites {
        heap: l.heap,
        data: l.data,
        room: l.room,
        revision: l.revision,
        since: l.since,
        written: l.written,
    }
}

/// Each square the windows want to `write`, while it takes them: off the
/// packets kept for them, and off each void tile, held with none.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let terrain = TerrainTile::residency();
    kept().send(pxl_render::gpu::api::device_generation(), &mut |frame, pkt| {
        let Some(t) = terrain.in_frame(frame) else { return Offered::Taken };
        let Some(held) = terrain.get(t) else { return Offered::Taken };
        window::fill(t, Handed::Packet(pkt, held), write)
    });
    // The due squares' void tiles, not every tile held.
    for p in window::voids() {
        if window::fill(p, Handed::Void, write) == Offered::Full {
            return;
        }
    }
}
