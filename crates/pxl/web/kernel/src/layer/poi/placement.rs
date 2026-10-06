// pxl-kernel/layer/poi/placement.rs

//! Where the POI counts are drawn: one badge per POI-lattice cell under each
//! drawn chunk, read from the tile that settles that ground,
//! standing at a leaf cell's center on the surface a ray down meets first.

use crate::control::camera::controller::units_per_px_at;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::field::billboard_instance;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::{enabled, rank_of, ALL_UNITS_PER_PX, MAJOR_UNITS_PER_PX};
use crate::param::POI_MAJOR_RANK;
use crate::sched::tick::TickContext;
use crate::world::access::{Access, Detail, Ray};
use crate::world::frame::view::ndc_to_px;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::read::TileRead;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;
use pxl_spec::space::zoom::nearest;
use xpute_runtime::global::Global;

use pxl_spec::bake::poi_tile::{PoiTile, POI_LATTICE};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{projection, REACH};
use pxl_spec::space::pyramid::{Cell, GROUND, LEAF_Z};
use pxl_spec::space::v3::len3;
use pxl_spec::space::zoom::{count, Zoom};
use std::collections::HashSet;

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, label, and where it stands.
    hits: Vec<([f64; 4], Label, Pvf3)>,
    /// Kept so a settled turn asks the heap for nothing. Each with the scale
    /// it shows at and where in its POI cell that scale is read.
    found: Vec<(Cell, Label, Cell, f64, Shown, [f64; 3])>,
    /// The drawn chunks, the tiles and the heights `found` was read from: an
    /// eye move that changes none of them casts no ray.
    found_for: Option<(u32, u32, (u32, u32))>,
    mounted: Vec<(Chunk, bool, bool)>,
    labeled: Vec<(Label, Pvf3)>,
    instances: Vec<f32>,
}
static PLACEMENT: Global<Placement> = Global::new();

/// Every place, or only the prominent: what a cell shows by the units a
/// logical pixel spans there.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    All,
    Major,
}

fn shown_at(units_per_px: f64) -> Option<Shown> {
    if units_per_px <= ALL_UNITS_PER_PX {
        Some(Shown::All)
    } else if units_per_px <= MAJOR_UNITS_PER_PX {
        Some(Shown::Major)
    } else {
        None
    }
}

/// None where nothing over it is known.
fn top_at(at: &Pvf3, zoom: Zoom) -> Option<f64> {
    let xy = projection::unproject(at.i, at.f);
    let (high, low) = (REACH.max[2], REACH.min[2]);
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: high - low,
    };
    let hit = Access::now().first_hit(&ray, Detail::Zoom(zoom)).value().flatten()?;
    Some(high - hit.t)
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if !enabled() || crate::world::observation::eye().is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.hits.clear();
        badge::retain(&HashSet::new());
        return;
    }
    let tiles = PoiTile::pyramid();
    if crate::world::pyramid::domain::current().is_none() {
        return;
    }
    let sched = &crate::kernel::kernel().sched;
    let key = Some((sched.drawn_revision(), tiles.residency().revision(), crate::world::observation::heights_revision()));
    let stale = p.found_for != key;
    p.found_for = key;
    let mut found = core::mem::take(&mut p.found);
    if stale {
        found.clear();
        sched.mounted(&mut p.mounted);
    }
    let read: &[(Chunk, bool, bool)] = if stale { &p.mounted } else { &[] };
    for &(chunk, _, _) in read.iter().filter(|m| m.1) {
        let read = TileRead::of(POI_LATTICE, chunk);
        let Some(src) = tiles.residency().source(read.tile()) else { continue };
        let Some(poi) = tiles.residency().get(src) else { continue };
        // Where the source cell is wider than the chunk, its place counts only if it
        // stands on the chunk's ground.
        let (side, cell) = (chunk.column().0, POI_LATTICE.cell_zoom(src.zoom));
        let m = if cell >= side { count(cell, side) } else { 0 };
        let [x0, y0] = POI_LATTICE.cell_of(src.zoom, &Pvf3 { i: chunk.corner(), f: 0 }).in_tile();
        let (h0, h1) = chunk.z_range();
        for ty in y0..y0 + m.max(1) {
            let (lo, hi) = (POI_LATTICE.cell_index(x0, ty), POI_LATTICE.cell_index(x0 + m.max(1), ty));
            let from = poi.cells.partition_point(|c| c.cell < lo);
            for c in poi.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = POI_LATTICE.cell_at(src, c.cell);
                // Near, a place or a count; farther, only a prominent enough place.
                // A count stands at its cell's middle.
                let all = match (c.count, poi.shown(c).next()) {
                    (1, Some((place, _, at))) => (Label::Place { tile: src, place }, cell.inner_at(GROUND, LEAF_Z, at)),
                    _ => (Label::Count(c.count, c.partial), GROUND.cell_of(LEAF_Z, &cell.mid())),
                };
                let major = poi
                    .shown(c)
                    .map(|(i, p, at)| (i, rank_of(p.kind), at))
                    .filter(|s| s.1 <= POI_MAJOR_RANK)
                    .min_by_key(|s| s.1)
                    .map(|(place, _, at)| (Label::Place { tile: src, place }, cell.inner_at(GROUND, LEAF_Z, at)));
                let mid = cell.mid();
                let mid = projection::unproject(mid.i, mid.f);
                for (shown, (label, ground)) in [(Shown::All, all)].into_iter().chain(major.map(|m| (Shown::Major, m))) {
                    if m == 0 && Chunk::holding(chunk.zoom(), &ground.mid()).column() != chunk.named().column() {
                        continue;
                    }
                    // Counted only by the chunk the top of what stands there is in, so a
                    // column's several chunks do not repeat it.
                    let Some(top) = top_at(&ground.mid(), read.tile_zoom()) else { continue };
                    if top - 1.0 < h0 || top - 1.0 >= h1 {
                        continue;
                    }
                    found.push((POI_LATTICE.cell_of(read.tile_zoom(), &ground.mid()), label, ground, top, shown, [mid[0], mid[1], top]));
                }
            }
        }
    }
    found.sort_unstable_by_key(|f| f.0);
    p.batch.reset();
    let mut labeled = core::mem::take(&mut p.labeled);
    labeled.clear();
    let eye = crate::world::observation::eye().unwrap_or_default();
    for &(_, label, stands, top, shown, at) in &found {
        // Found once while it stands, so its lap is chosen as the eye moves.
        let d = len3(&[nearest(at[0], eye[0]) - eye[0], at[1] - eye[1], at[2] - eye[2]]);
        if shown_at(units_per_px_at(d)) != Some(shown) {
            continue;
        }
        let Some(width) = badge::width(label) else { continue };
        let center = stands.center(top);
        let local = crate::world::frame::coord::to_world(&center);
        p.batch.push(local[0], local[1], local[2], width, badge::ANCHOR, labeled.len() as f64);
        labeled.push((label, center));
    }
    badge::retain(&labeled.iter().map(|l| l.0).collect());
    p.found = found;
    select_visible_billboards(&mut p.batch, &mut p.kept);
    p.drawn.clear();
    for &i in &p.kept {
        if badge::slot(labeled[i as usize].0) == NO_SLOT {
            continue;
        }
        p.drawn.push(i);
    }
    // Farthest first: the pass writes no depth, so the nearer must draw last.
    let batch = &p.batch;
    p.drawn.sort_by(|&a, &b| {
        batch.d2[b as usize]
            .total_cmp(&batch.d2[a as usize])
            .then(batch.tiebreak[a as usize].total_cmp(&batch.tiebreak[b as usize]))
    });
    p.hits.clear();
    p.hits.extend(p.drawn.iter().rev().map(|&i| (p.batch.screen[i as usize], labeled[i as usize].0, labeled[i as usize].1)));
    let mut instances = core::mem::take(&mut p.instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = crate::world::frame::view::sea_under_eye();
    for &i in &p.drawn {
        let i = i as usize;
        let at = [p.batch.local_x[i] - about[0], p.batch.local_y[i] - about[1], p.batch.local_z[i] - about[2]];
        instances.extend_from_slice(&billboard_instance(
            at,
            [p.batch.w_px[i] as f64, badge::HEIGHT],
            badge::slot(labeled[i].0) as u32,
            badge::ANCHOR / badge::HEIGHT,
        ));
    }
    if p.set.is_some() || !instances.is_empty() {
        let set = *p.set.get_or_insert_with(|| new_billboard_set(BillboardOrder::Poi));
        place_billboard_set(set, about);
        write_billboard_set(set, &instances);
        ctx.invalidate();
    }
    (p.labeled, p.instances) = (labeled, instances);
}

/// `ndc` in [−1, 1], y up. A count is also zoomed into, so it comes apart
/// into its places. Whether a badge took the tap.
pub fn tap(ndc_x: f64, ndc_y: f64) -> bool {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    let Some(&(_, label, stands)) = hits.iter().find(|(r, _, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y)) else {
        return false;
    };
    match label {
        Label::Count(..) => crate::shell::shell::go_into(stands),
        _ => crate::shell::shell::go_to(stands),
    }
    true
}
