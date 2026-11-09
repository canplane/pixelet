// pxl-core/domain/scene/cast.rs

//! What the GPU's cast of the eye's rays reads from the core (pxl-render
//! ray/cast.rs `Cast`): the eye and the service area's index. The layers' parts
//! of the cast are their own: the clipmap's table of bricks is written by
//! domain/layer/terrain/clipmap.rs, the placed objects by
//! domain/layer/object/store.rs `objects`.

use pxl_engine::eye;
use pxl_spec::space::camera::eye::Observation;
use xpute_core::global::Global;

use crate::domain::preview::RenderEdit;
use crate::domain::view::lens;
use crate::sys::io::source;
use pxl_render::gpu::api;
use pxl_spec::space::layer::octant::Octant;

/// The service area's index as the GPU reads it, and whether it was built
/// after a source was declared. A source is declared once (sys/io/source.rs
/// `declare`), so the index is built once before that and once after.
static SHARDS: Global<(bool, Vec<[i32; 4]>)> = Global::new();

/// The service area (pxl-spec `area::Area`) in rows of four words. The word
/// at a cell's key (`area::shard_key`) holds that cell's nine: which of the
/// nine cells about it are served. Before a source is declared the index is
/// one row of zeros, because a GPU binding cannot be empty, and a read past
/// its end is zero as well. The renderer hands the GPU this memory where it
/// lies, so the index is not rebuilt while the runtime holds it (pxl-render
/// gpu/api.rs `in_flight`); a source declared meanwhile is built in at a
/// later call.
pub fn shards() -> &'static [[i32; 4]] {
    let declared = source::min_zoom().is_some();
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

/// The eye's words for the GPU's cast (`lens::gpu_cast`), with the pointer's
/// preview `edit` added so that the GPU's world answers as the CPU's does
/// while previewing (domain/query/world.rs `previewing`). The preview's carve
/// is cut to air and its hidden object is left out; each is written as X, Y,
/// Z and the zoom (pxl-engine `eye`). `steps` is the `ray_steps` setting,
/// written as the ray steps view's word (`eye::STEPS`). None without a screen.
pub fn eye(o: &Observation, edit: &RenderEdit, steps: bool) -> Option<[u32; eye::WORDS]> {
    let mut words = [0u32; eye::WORDS];
    lens::gpu_cast(o, &mut words)?;
    words[eye::STEPS] = u32::from(steps);
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
    words[eye::CARVE..eye::CARVE + eye::CUT_WORDS].copy_from_slice(&cut(edit.carve));
    words[eye::HIDE..eye::HIDE + eye::CUT_WORDS].copy_from_slice(&cut(edit.hide));
    Some(words)
}
