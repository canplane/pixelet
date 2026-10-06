// pxl-kernel/control/navigation/support.rs

//! The map center standing on the ground, and where every camera's pan
//! puts it. The center's Z is half a cell over the floor the character
//! stands on, as a cell's center stands over its ground. Only a leaf gives
//! that floor; a center put over coarser ground keeps its Z unknown and
//! stands on the finest ground known there until the leaves' is.

use pxl_spec::space::coord::pvf3::core::{FRAC_W, IVEC_Z_W};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::shell::setting::store::settings_store;
use crate::world::access::{Access, Answer, Detail, Ray};
use crate::world::frame::coord::{to_world, world_to_canonical};
use pxl_spec::space::coord::zoom::{index_of, units};

/// The Z lane's floor, under any ground.
const UNKNOWN_Z: i64 = -(1 << (IVEC_Z_W - 1)) << FRAC_W;

fn with_z(at: &Pvf3, z: i64) -> Pvf3 {
    pvf3::gather(pvf3::X(at), pvf3::Y(at), z)
}

pub fn unknown_z(at: &Pvf3) -> Pvf3 {
    with_z(at, UNKNOWN_Z)
}

pub fn is_unknown(at: &Pvf3) -> bool {
    pvf3::Z(at) == UNKNOWN_Z
}

/// A tap names the surface it struck: the center stands in the cell across
/// that face, on the floor under it.
pub fn stand(at: &Pvf3) -> Pvf3 {
    match Access::now().floor_under(at) {
        Answer::Exact(Some(top)) => stand_over(at, top),
        // Nothing to stand on: the point keeps its height.
        Answer::Exact(None) => *at,
        Answer::Coarser(..) | Answer::Nothing(_) => unknown_z(at),
    }
}

/// The center's height over a floor whose top is `top`.
pub fn over(top: f64) -> f64 {
    top + units(GROUND.cell()) / 2.0
}

/// The top of the floor `center` stands over.
pub fn floor_of(center: &Pvf3) -> f64 {
    pvf3::to_real(pvf3::Z(center)) - units(GROUND.cell()) / 2.0
}

fn stand_over(at: &Pvf3, top: f64) -> Pvf3 {
    with_z(at, pvf3::from_real(over(top)))
}

/// An unknown Z on the surface of the finest ground known there; none where
/// nothing stands.
fn standing(at: &Pvf3) -> Answer<Option<Pvf3>> {
    if !is_unknown(at) {
        return Answer::Exact(Some(*at));
    }
    Access::now().surface_support(at).map(|top| top.map(|top| stand_over(at, top)))
}

/// Where `at` is drawn; at `held`'s height where nothing is known.
pub fn resolve(at: &Pvf3, held: V3) -> V3 {
    match standing(at) {
        Answer::Exact(Some(p)) | Answer::Coarser(Some(p), _) => to_world(&p),
        Answer::Exact(None) | Answer::Coarser(None, _) | Answer::Nothing(_) => {
            let l = to_world(at);
            [l[0], l[1], held[2]]
        }
    }
}

/// An unknown Z once a leaf gives it.
pub fn exact(at: &Pvf3) -> Option<Pvf3> {
    standing(at).exact().flatten()
}

/// `resolve` for a camera's target, writing the leaves' Z into it once known.
pub fn target(to: &mut Option<Pvf3>, held: V3) -> V3 {
    let Some(at) = *to else { return held };
    if let Some(p) = exact(&at) {
        *to = Some(p);
    }
    resolve(&to.unwrap_or(at), held)
}

/// By `passage`, then down to the floor.
pub fn pan_to(from: &Pvf3, dx: f64, dy: f64, units_per_px: f64, azimuth: f64) -> Pvf3 {
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = libm::sincos(azimuth);
    let l = to_world(from);
    let way = [lx * c + ly * s, -lx * s + ly * c];
    let to = world_to_canonical(&[l[0] + way[0], l[1] + way[1], l[2]]);
    if is_unknown(from) {
        return unknown_z(&to);
    }
    if !settings_store().surface_follow {
        return to;
    }
    if way == [0.0, 0.0] {
        return *from;
    }
    let mut at = to;
    match passage(from, way, index_of(GROUND.cell(), from)[2]) {
        Passage::Open(cz) => at = in_cell(&to, cz),
        Passage::Unknown => {}
        Passage::Blocked => return *from,
    }
    match Access::now().floor_under(&at) {
        Answer::Exact(Some(top)) => stand_over(&at, top),
        Answer::Exact(None) => at,
        _ => unknown_z(&at),
    }
}

/// `at` in cell height `cz`.
pub fn in_cell(at: &Pvf3, cz: i64) -> Pvf3 {
    stand_over(at, cz as f64 * units(GROUND.cell()))
}

pub enum Passage {
    Open(i64),
    /// Open sky, way not known exactly.
    Unknown,
    Blocked,
}

/// Lowest open cell height from `start` along `way`: over each blocker, under `from`'s ceiling.
pub fn passage(from: &Pvf3, way: [f64; 2], start: i64) -> Passage {
    let access = Access::now();
    let Some(ceiling) = access.ceiling_over(from).exact() else { return Passage::Blocked };
    let length = libm::hypot(way[0], way[1]);
    let side = units(GROUND.cell());
    let [x, y] = [pvf3::to_real(pvf3::X(from)), pvf3::to_real(pvf3::Y(from))];
    let dir = if length > 0.0 { [way[0] / length, way[1] / length] } else { [0.0, 0.0] };
    let mut cz = start;
    // Decor is passed.
    let mut t = 0.0;
    loop {
        if ceiling.is_some_and(|top| cz >= top) {
            return Passage::Blocked;
        }
        let ray = Ray {
            origin: [x + dir[0] * t, y + dir[1] * t, over(cz as f64 * side)],
            direction: [dir[0], dir[1], 0.0],
            reach: length - t,
        };
        match access.first_hit(&ray, Detail::Zoom(LEAF_ZOOM)) {
            Answer::Exact(None) => return Passage::Open(cz),
            Answer::Exact(Some(hit)) => {
                let p = ray.at(hit.t + side / 2.0);
                let cell = pvf3::gather(pvf3::from_real(p[0]), pvf3::from_real(p[1]), pvf3::from_real(p[2]));
                match access.open_from(&cell).exact() {
                    Some(open) if open > cz => (cz, t) = (open, 0.0),
                    Some(_) => t += hit.t + side,
                    None => return Passage::Blocked,
                }
            }
            _ if ceiling.is_none() => return Passage::Unknown,
            _ => return Passage::Blocked,
        }
    }
}
