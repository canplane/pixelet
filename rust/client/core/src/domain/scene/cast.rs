// pxl-core/domain/scene/cast.rs

//! What the GPU's cast of the eye's rays reads (pxl-render gpu/ray/cast.rs
//! `Cast`): the eye as the rays leave it, a ray a drawn pixel, and the eye's
//! clip levels as a table of the bricks each holds, each brick handed to the
//! GPU once it is filled (domain/layer/terrain/clipmap.rs `upload`).

use pxl_engine::eye;
use pxl_render::gpu::ray::Overwrites;
use pxl_spec::space::camera::eye::Observation;
use xpute_core::global::Global;

use crate::domain::layer::object::store::object;
use crate::domain::preview::RenderEdits;
use crate::domain::view::lens;
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
/// renderer hands it where it lies, so a source declared while the runtime
/// holds it is made at a later call (pxl-render gpu/api.rs `in_flight`).
pub fn shards() -> &'static [[i32; 4]] {
    let declared = source::top().is_some();
    let (made, s) = SHARDS.get(|| (false, Vec::new()));
    if s.is_empty() || (*made != declared && !api::in_flight(s)) {
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

/// The eye as the GPU's rays leave it, a ray a drawn pixel (`lens::gpu_cast`),
/// then `edits`, the pointer's preview, as the world answers through it
/// (domain/query/world.rs `previewing`), its cut to air and its left-out leaf,
/// each X, Y, Z and the zoom (pxl-engine `eye`). None without a screen.
pub fn eye(o: &Observation, edits: &RenderEdits) -> Option<[u32; eye::WORDS]> {
    let mut words = [0u32; eye::WORDS];
    lens::gpu_cast(o, &mut words)?;
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
