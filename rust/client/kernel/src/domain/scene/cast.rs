// pxl-kernel/domain/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/ray/cast.rs
//! `Cast`): the eye as the rays leave it, a ray a drawn pixel, the eye's
//! clip levels as a table of the bricks each holds, and each brick the
//! clip levels want handed to the GPU once, off its tile's packet
//! (domain/layer/terrain/clipmap.rs `fill`).

use pxl_engine::eye;
use pxl_render::gpu::ray::{Overwrites, TerrainWrite};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use xpute_guest::global::Global;

use crate::domain::layer::object::preview;
use crate::domain::layer::object::store::object;
use crate::domain::layer::terrain::clipmap::{self, TileContent};
use crate::domain::layer::terrain::tile::uploads;
use crate::domain::layer::upload::Uptake;
use crate::domain::layer::Base as _;
use crate::domain::view::frame::resolution;
use crate::domain::view::lens;
use crate::domain::view::setting::store::settings_store;
use crate::sys::io::source;
use pxl_render::gpu::api;
use pxl_spec::space::layer::octant::Octant;

/// The service area's index as the GPU reads it, and whether a declared
/// source made it: one is declared once (sys/io/source.rs `declare`), so it is
/// made then and not again.
static SHARDS: Global<(bool, Vec<[i32; 4]>)> = Global::new();

/// The area (pxl-engine `area::Area`): every cell's nine by its key
/// (`area::shard_key`), four a row; a row of none where none is declared,
/// since a binding holds at least one, and a read past it is none. The
/// renderer hands it where it lies, so a source declared while the host
/// holds it is made at a later call (pxl-render gpu/api.rs `held`).
pub fn shards() -> &'static [[i32; 4]] {
    let declared = source::top().is_some();
    let (made, s) = SHARDS.get(|| (false, Vec::new()));
    if s.is_empty() || (*made != declared && !api::held(s)) {
        s.clear();
        let nines = source::nines();
        let nine = |i: usize| nines.get(i).copied().map_or(0, i32::from);
        s.extend(
            (0..nines.len().div_ceil(4).max(1)).map(|r| core::array::from_fn(|k| nine(4 * r + k))),
        );
        *made = declared;
    }
    s
}

/// The eye as the GPU's rays leave it, a ray a drawn pixel, its beam rays
/// on the lattice the drawn resolution leaves (`lens::gpu_cast`,
/// domain/view/frame/resolution.rs `lattice`),
/// then the pointer's preview as the world answers through it (domain/query/world.rs
/// `previewing`), its cut to air and its left-out leaf, each X, Y, Z and the
/// zoom (pxl-engine `eye`). None without a screen.
pub fn eye(o: &Observation) -> Option<[u32; eye::WORDS]> {
    let mut words = [0u32; eye::WORDS];
    lens::gpu_cast(
        o,
        settings_store().ray_steps,
        resolution::lattice(),
        &mut words,
    )?;
    let edits = preview::cur();
    let cut = |e: Option<Octant>| {
        e.map_or([0, 0, 0, eye::NO_CUT], |e| {
            [
                e.at()[0] as u32,
                e.at()[1] as u32,
                e.at()[2] as u32,
                e.zoom(),
            ]
        })
    };
    words[eye::CARVE..eye::CARVE + eye::CUT_WORDS].copy_from_slice(&cut(edits.carve));
    words[eye::HIDE..eye::HIDE + eye::CUT_WORDS].copy_from_slice(&cut(edits.hide));
    Some(words)
}

/// What is placed, for the GPU: the objects' tree's lanes as they live, the
/// room either has, the store's revision, and the child groups written since the
/// revision the lanes were last handed at, so the GPU takes only those
/// again where it holds that one (domain/layer/object/store.rs `gpu_columns`).
pub fn overwrites() -> Overwrites<'static> {
    let l = object().gpu_columns();
    Overwrites {
        first_child: l.first_child,
        data: l.data,
        room: l.room,
        revision: l.revision,
        since: l.since,
        written_groups: l.written_groups,
    }
}

/// Each brick the clip levels want to `write`, while it takes them: off the
/// packets waiting for them, and off each void tile, held with none.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let terrain = TerrainTile::tile_cache();
    let wanted = |slot| {
        terrain
            .in_slot(slot)
            .is_some_and(|t| terrain.get(t).is_some() && clipmap::is_tile_wanted(t))
    };
    uploads().send(
        api::device_generation(),
        &mut |slot, pkt| {
            let Some(t) = terrain.in_slot(slot) else {
                return Uptake::Taken;
            };
            let Some(held) = terrain.get(t) else {
                return Uptake::Taken;
            };
            clipmap::fill(t, TileContent::Packet(pkt, held), write)
        },
        &wanted,
    );
    // The due bricks' void tiles, not every tile held.
    for p in clipmap::voids() {
        if clipmap::fill(p, TileContent::Void, write) == Uptake::Full {
            return;
        }
    }
}
