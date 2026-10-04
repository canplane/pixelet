// pxl-kernel/layer/poi/placement.rs

//! Where the POI counts are drawn: one badge per POI-lattice cell under each
//! drawn chunk, read from the page that settles that ground,
//! standing at a z16 cell's center on the surface a ray down meets first.

use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::field::billboard_instance;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::{enabled, rank_of};
use crate::param::{POI_ALL_LEVEL, POI_MAJOR_RANK};
use crate::sched::tick::TickContext;
use crate::world::access::{Access, Detail, Ray};
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::ndc_to_px;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;
use xpute_runtime::global::Global;

use pxl_spec::bake::poi_page::{PoiPage, POI_LATTICE};
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
    /// What a turn works in, kept so a settled turn asks the heap for
    /// nothing: each badge found, by the cell of the piece's level it stands
    /// in — the order the badges are listed in — with its label and the z16
    /// cell and height it stands at; each badge's label and where it stands;
    /// and the instances written for the set.
    found: Vec<(Cell, Label, Cell, f64)>,
    /// The drawn chunks, the pages and the heights `found` was read from: an
    /// eye move that changes none of them casts no ray.
    found_for: Option<(u32, u32, (u32, u32))>,
    mounted: Vec<(Chunk, bool, bool)>,
    labeled: Vec<(Label, Pvf3)>,
    instances: Vec<f32>,
}
static PLACEMENT: Global<Placement> = Global::new();

/// The top of what stands at `at`'s X and Y as the view draws it at `level`:
/// where a ray straight down first meets the ground or a placed object.
/// None where nothing over it is known.
fn top_at(at: &Pvf3, level: u32) -> Option<f64> {
    let mut xy = [0.0; 3];
    projection::unproject(&mut xy, at.i, at.f);
    let (high, low) = (REACH.max[2], REACH.min[2]);
    let ray = Ray { origin: [xy[0], xy[1], high], direction: [0.0, 0.0, -1.0], reach: high - low };
    let hit = Access::now().first_hit(&ray, Detail::Level(level)).value().flatten()?;
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
    let origin = view().origin;
    let base = ivec::to_v3(origin);
    let pages = PoiPage::pyramid();
    // No pieces made yet: what was placed stands.
    if crate::world::pyramid::domain::current().is_none() {
        return;
    }
    let sched = &crate::kernel::kernel().sched;
    let key = Some((sched.drawn_revision(), pages.paging().revision(), crate::world::observation::heights_revision()));
    let stale = p.found_for != key;
    p.found_for = key;
    let mut found = core::mem::take(&mut p.found);
    found.clear();
    for &chunk in &crate::world::pyramid::cover::cover::<PoiPage>().0.drawn {
        // None coarser than COARSEST, only the most prominent up to POI_ALL_LEVEL.
        if chunk.level() > PoiPage::COARSEST {
            continue;
        }
        let major_only = chunk.level() > POI_ALL_LEVEL;
        // The page that draws the chunk's ground: its own, or where the tree
        // stops above it, the page over it that says that ground — whose cells
        // there hold one place or none, each standing where it stands.
        let Some(src) = pages.source(chunk.page()) else { continue };
        let Some(poi) = pages.get(src) else { continue };
        // The chunk's ground in the source's cells: a block of them, or where
        // the source is so much coarser that one of its cells is wider than
        // the chunk, part of one, whose place is the chunk's only if it stands
        // on the chunk's ground.
        let (side, cell) = (chunk.column().0, POI_LATTICE.scale(src.level));
        let m = if cell <= side { cell.count_in(side) } else { 0 };
        let [x0, y0] = POI_LATTICE.cell_of(src.level, &Pvf3 { i: chunk.corner(), f: 0 }).in_page();
        let (z0, z1) = chunk.z_range();
        for ty in y0..y0 + m.max(1) {
            let (lo, hi) = (POI_LATTICE.cell_index(x0, ty), POI_LATTICE.cell_index(x0 + m.max(1), ty));
            let from = poi.cells.partition_point(|c| c.cell < lo);
            for c in poi.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = POI_LATTICE.cell_at(src, c.cell);
                // What the cell is shown as, which is the reader's to choose
                // (layer/poi.rs `rank_of`): every cell from `POI_ALL_LEVEL`, a
                // place where it holds one and a count where it holds more,
                // as the clusters of a map's markers are; coarser, only the
                // most prominent place it holds, where that is prominent
                // enough. A place stands where it does, a count at its cell's
                // middle.
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
                // A column is drawn by every chunk over it that holds
                // something (world/pyramid/chunk.rs); a place is counted by
                // the one the top of what stands on its ground is in, and by
                // none where nothing under it is known, where no badge could
                // stand.
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
        let at = [p.batch.local_x[i], p.batch.local_y[i], p.batch.local_z[i]];
        instances.extend_from_slice(&billboard_instance(
            at,
            [p.batch.w_px[i] as f64, badge::HEIGHT],
            badge::slot(labeled[i].0) as u32,
            badge::ANCHOR / badge::HEIGHT,
        ));
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
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    let Some(&(_, label, stands)) = hits.iter().find(|(r, _, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y)) else {
        return false;
    };
    match label {
        // A count opens up as it is looked at closer.
        Label::Count(..) => crate::control::camera::controller::focus(stands),
        _ => crate::shell::shell::go_to(stands),
    }
    true
}
