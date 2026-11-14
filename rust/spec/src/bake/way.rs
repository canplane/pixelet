// pxl-spec/bake/way.rs

//! A way covers the cells along its line by way.rs's rule (`on_way`, `reach2`), on
//! the ground and on a deck alike. An approach's asset id is laid into a leaf's cells
//! (`lay`), so every zoom folds it as the ground's; its embankment or cutting stays a line.

use fnv::FnvHashMap;

use super::separation::{Form, Separation};
use crate::asset::cube::{Ground, Sentinel};
use crate::ground::GroundId;
use crate::procedural::Frame;
use crate::space::layer::lattice::TILE_CELLS;

/// Lays an approach's lines into the asset ids of a leaf at `frame`: each cell a line
/// crosses takes the way's asset_id, the highest way's where two cross one. Bedrock and kinds that
/// do not ground are left as they are, as the ground's face takes no way there; an
/// embankment's or a cutting's height stays its line's.
pub fn lay(sep: &Separation, frame: Frame, asset_ids: &mut [GroundId]) {
    let n = i64::from(TILE_CELLS);
    // For each cell, the highest top over it and its asset_id.
    let mut laid: FnvHashMap<usize, (i32, u8)> = FnvHashMap::default();
    sep.crossing_grid(frame, [0, 0], [n, n], |x, y, _, o| {
        let col = (y * n + x) as usize;
        if o.form != u32::from(Form::Approach.byte())
            || asset_ids[col] == Sentinel::Unknown as GroundId
            || u8::try_from(o.asset_id).ok().and_then(Ground::of).is_none()
        {
            return;
        }
        let e = laid.entry(col).or_insert((i32::MIN, o.asset_id as u8));
        if o.top > e.0 {
            *e = (o.top, o.asset_id as u8);
        }
    });
    for (col, (_, asset_id)) in laid {
        asset_ids[col] = asset_id;
    }
}
