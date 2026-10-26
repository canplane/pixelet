// pxl-kernel/domain/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/renderer.rs
//! `Cast`): the eye as the rays leave it, a ray a drawn pixel, the eye's
//! windows as a table of the squares each holds, and each square the
//! windows want handed to the GPU once, off its tile's packet
//! (domain/layer/terrain/window.rs `fill`).

use pxl_render::gpu::renderer::{Overwrites, TerrainWrite};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec_core::eye;
use xpute_guest::global::Global;

use crate::domain::layer::kept::Offered;
use crate::domain::layer::object::preview;
use crate::domain::layer::object::store::objects;
use crate::domain::layer::terrain::tile::kept;
use crate::domain::layer::terrain::window::{self, Handed};
use crate::domain::layer::Base as _;
use crate::domain::view::lens;
use crate::domain::view::setting::store::{settings_store, OracleStart, ProxyCut};
use pxl_spec::bake::constant::EntryAt;

/// The cut a proxy draws, as the setting asks for it: its `coarser`
/// (pxl-spec-core `proxy::cut`), none for 0, and whether it is drawn
/// (pxl-render gpu/renderer.rs `Cast::proxy`).
pub fn proxy() -> u32 {
    use pxl_render::gpu::ray::PROXY_RASTER;
    match settings_store().ray_proxy {
        ProxyCut::off => 0,
        ProxyCut::cut_1 => 1,
        ProxyCut::cut_2 => 2,
        ProxyCut::cut_3 => 3,
        ProxyCut::raster_1 => 1 | PROXY_RASTER,
        ProxyCut::raster_2 => 2 | PROXY_RASTER,
        ProxyCut::raster_3 => 3 | PROXY_RASTER,
    }
}

/// The service area's index as the GPU reads it, and whether a declared
/// source made it: one is declared once (sys/io/source.rs `declare`), so it is
/// made then and not again.
static SHARDS: Global<(bool, Vec<[i32; 4]>)> = Global::new();

/// The area (pxl-spec-core `area::Area`): every cell's nine by its key
/// (`area::shard_key`), four a row; a row of none where none is declared,
/// since a binding holds at least one, and a read past it is none.
pub fn shards() -> &'static [[i32; 4]] {
    let declared = crate::sys::io::source::top().is_some();
    let (made, s) = SHARDS.get(|| (false, Vec::new()));
    if s.is_empty() || *made != declared {
        s.clear();
        let nines = crate::sys::io::source::nines();
        let nine = |i: usize| nines.get(i).copied().map_or(0, i32::from);
        s.extend(
            (0..nines.len().div_ceil(4).max(1)).map(|r| core::array::from_fn(|k| nine(4 * r + k))),
        );
        *made = declared;
    }
    s
}

/// The eye as the GPU's rays leave it, a ray a drawn pixel (`lens::gpu_cast`),
/// then the pointer's preview as the world answers through it (domain/query/world.rs
/// `previewing`), its cut to air and its left-out leaf, each X, Y, Z and the
/// zoom (pxl-spec-core `eye`). None without a screen.
pub fn eye(o: &Observation) -> Option<[u32; eye::WORDS]> {
    let mut words = [0u32; eye::WORDS];
    let s = settings_store();
    let (oracle, locate) = match s.ray_oracle {
        OracleStart::off => (eye::ORACLE_OFF, 0),
        OracleStart::root => (eye::ORACLE_ROOT, 0),
        OracleStart::cut_1 => (eye::ORACLE_CUT + 1, 0),
        OracleStart::cut_2 => (eye::ORACLE_CUT + 2, 0),
        OracleStart::cut_3 => (eye::ORACLE_CUT + 3, 0),
        OracleStart::cut_4 => (eye::ORACLE_CUT + 4, 0),
        OracleStart::cut_6 => (eye::ORACLE_CUT + 6, 0),
        OracleStart::cut_8 => (eye::ORACLE_CUT + 8, 0),
        OracleStart::locate_1 => (eye::ORACLE_CUT + 1, eye::ORACLE_LOCATE),
        OracleStart::locate_2 => (eye::ORACLE_CUT + 2, eye::ORACLE_LOCATE),
        OracleStart::loose_1 => (eye::ORACLE_CUT + 1, eye::ORACLE_LOOSE),
        OracleStart::loose_2 => (eye::ORACLE_CUT + 2, eye::ORACLE_LOOSE),
        OracleStart::eye_1 => (eye::ORACLE_CUT + 1, eye::ORACLE_FROM_EYE),
        OracleStart::eye_2 => (eye::ORACLE_CUT + 2, eye::ORACLE_FROM_EYE),
    };
    let steps = (u32::from(s.ray_steps) * eye::SHOW_STEPS)
        | (oracle << eye::ORACLE_SHIFT)
        | locate
        | (u32::from(s.ray_proxied) * eye::PROXIED);
    lens::gpu_cast(o, lens::tau(), steps, &mut words)?;
    let edits = preview::current();
    let cut = |e: Option<EntryAt>| {
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
/// room either has, the store's revision, and the blocks written since the
/// revision the lanes were last handed at, so the GPU takes only those
/// again where it holds that one (domain/layer/object/store.rs `gpu_lanes`).
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
    let wanted = |frame| {
        terrain
            .in_frame(frame)
            .is_some_and(|t| terrain.get(t).is_some() && window::tile_wanted(t))
    };
    kept().send(
        pxl_render::gpu::api::device_generation(),
        &mut |frame, pkt| {
            let Some(t) = terrain.in_frame(frame) else {
                return Offered::Taken;
            };
            let Some(held) = terrain.get(t) else {
                return Offered::Taken;
            };
            window::fill(t, Handed::Packet(pkt, held), write)
        },
        &wanted,
    );
    // The due squares' void tiles, not every tile held.
    for p in window::voids() {
        if window::fill(p, Handed::Void, write) == Offered::Full {
            return;
        }
    }
}
