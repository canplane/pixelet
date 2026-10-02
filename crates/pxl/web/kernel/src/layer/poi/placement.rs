// pxl-kernel/layer/poi/placement.rs

//! Where the POI counts are drawn: one badge per POI-lattice cell under each
//! piece of the layer's cover, read from the page that settles that ground,
//! standing at a z16 cell's center on the surface a ray down meets first.

use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::{enabled, rank_of};
use crate::param::{POI_ALL_LEVEL, POI_MAJOR_LEVEL, POI_MAJOR_RANK};
use crate::sched::tick::TickContext;
use crate::world::access::{Access, Detail, Ray};
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::view;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;

use pxl_spec::bake::poi_page::{PoiPage, POI_LATTICE};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{ivec, projection, REACH};
use pxl_spec::space::pyramid::{Cell, GROUND};
use std::collections::HashSet;

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, label, and where it stands.
    hits: Vec<([f64; 4], Label, Pvf3)>,
    /// Kept so a settled turn asks the heap for nothing.
    found: Vec<(Cell, Label, Cell, f64)>,
    labeled: Vec<(Label, Pvf3)>,
    instances: Vec<f32>,
}
static PLACEMENT: Global<Placement> = Global::new();

/// None where nothing over it is known.
fn top_at(at: &Pvf3, level: u32) -> Option<f64> {
    let mut xy = [0.0; 3];
    projection::unproject(&mut xy, at.i, at.f);
    let (high, low) = (REACH.max[2], REACH.min[2]);
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: high - low,
    };
    let hit = Access::now().first_hit(&ray, Detail::Level(level)).value().flatten()?;
    Some(high - hit.t)
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if !enabled() || crate::control::camera::controller::map_center().is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.hits.clear();
        badge::retain(&HashSet::new());
        return;
    }
    let origin = local_world_origin();
    let base = [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))];
    let pages = PoiPage::pyramid();
    if crate::world::pyramid::domain::current().is_none() {
        return;
    }
    let mut found = core::mem::take(&mut p.found);
    found.clear();
    for &chunk in &crate::world::pyramid::cover::cover::<PoiPage>().0.drawn {
        // None coarser than POI_MAJOR_LEVEL, only the most prominent up to POI_ALL_LEVEL.
        if chunk.level() > POI_MAJOR_LEVEL {
            continue;
        }
        let major_only = chunk.level() > POI_ALL_LEVEL;
        let Some(src) = pages.source(chunk.page()) else { continue };
        let Some(poi) = pages.get(src) else { continue };
        // Where the source cell is wider than the chunk, its place counts only if it
        // stands on the chunk's ground.
        let (side, cell) = (chunk.column().0, POI_LATTICE.scale(src.level));
        let m = if cell <= side { cell.count_in(side) } else { 0 };
        let [x0, y0] = POI_LATTICE.cell_of(src.level, &Pvf3 { i: chunk.corner(), f: 0 }).in_page();
        let (z0, z1) = chunk.z_range();
        for ty in y0..y0 + m.max(1) {
            let (lo, hi) = (POI_LATTICE.cell_index(x0, ty), POI_LATTICE.cell_index(x0 + m.max(1), ty));
            let from = poi.cells.partition_point(|c| c.cell < lo);
            for c in poi.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = POI_LATTICE.cell_at(src, c.cell);
                // From POI_ALL_LEVEL, a place or a count; coarser, only a prominent enough
                // place. A count stands at its cell's middle.
                let (label, ground) = if major_only {
                    let Some((place, _, at)) = poi.shown(c).map(|(i, p, at)| (i, rank_of(p.kind), at)).filter(|s| s.1 <= POI_MAJOR_RANK).min_by_key(|s| s.1) else {
                        continue;
                    };
                    (Label::Place { page: src, place }, cell.inner_at(GROUND, 0, at))
                } else {
                    match (c.count, poi.shown(c).next()) {
                        (1, Some((place, _, at))) => (Label::Place { page: src, place }, cell.inner_at(GROUND, 0, at)),
                        _ => (Label::Count(c.count, c.partial), GROUND.cell_of(0, &cell.mid())),
                    }
                };
                if m == 0 && Chunk::of(chunk.level(), &ground.mid()).column() != chunk.column() {
                    continue;
                }
                // Counted only by the chunk the top of what stands there is in, so a
                // column's several chunks do not repeat it.
                let Some(top) = top_at(&ground.mid(), chunk.level()) else { continue };
                if top - 1.0 < z0 || top - 1.0 >= z1 {
                    continue;
                }
                found.push((POI_LATTICE.cell_of(chunk.level(), &ground.mid()), label, ground, top));
            }
        }
    }
    found.sort_unstable_by_key(|f| f.0);
    p.batch.reset();
    let mut labeled = core::mem::take(&mut p.labeled);
    labeled.clear();
    for &(_, label, stands, top) in &found {
        let Some(width) = badge::width(label) else { continue };
        let center = stands.center(top);
        let local = crate::world::frame::coord::canonical_to_local(&center, origin);
        p.batch.push(local[0], local[1], local[2], width, badge::ANCHOR, labeled.len() as f64);
        labeled.push((label, center));
    }
    badge::retain(&labeled.iter().map(|l| l.0).collect());
    p.found = found;
    let v = view();
    select_visible_billboards(&mut p.batch, v.width, v.height, &mut p.kept);
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
    for &i in &p.drawn {
        let i = i as usize;
        let (sheet, slot) = (billboard_sheet(), badge::slot(labeled[i].0) as u32);
        instances.extend_from_slice(&[
            p.batch.local_x[i] as f32,
            p.batch.local_y[i] as f32,
            p.batch.local_z[i] as f32,
            p.batch.w_px[i],
            badge::HEIGHT as f32,
            sheet.u0(slot) as f32,
            sheet.v0(slot) as f32,
            sheet.u1(slot) as f32,
            sheet.v1(slot) as f32,
            (badge::ANCHOR / badge::HEIGHT) as f32,
        ]);
    }
    if p.set.is_some() || !instances.is_empty() {
        let set = *p.set.get_or_insert_with(|| new_billboard_set(BillboardOrder::Poi));
        place_billboard_set(set, base);
        write_billboard_set(set, &instances);
        ctx.invalidate();
    }
    (p.labeled, p.instances) = (labeled, instances);
}

/// `ndc` in [−1, 1], y up. A count is also zoomed into, so it comes apart
/// into its places. Whether a badge took the tap.
pub fn tap(ndc_x: f64, ndc_y: f64) -> bool {
    let v = view();
    let (x, y) = ((ndc_x * 0.5 + 0.5) * v.width, (-ndc_y * 0.5 + 0.5) * v.height);
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
