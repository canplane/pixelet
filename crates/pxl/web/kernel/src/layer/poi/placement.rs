// pxl-kernel/layer/poi/placement.rs

//! Where the counts are drawn. What the POI pages draw of the view is the
//! layer's own cover (world/pyramid/cover.rs `Cover`): pieces of the ground in
//! view, none over another, so every place under them is counted once. Each
//! of their cells — cells of the POI lattice (pxl-spec bake/poi_page.rs
//! `POI_LATTICE`) — is counted into itself, or into the cell over it at the
//! level the view wants there, where a finer page stands in for it: how
//! coarse places are is the camera's distance and nothing else, the ground's
//! the same way but apart from it. A cell holding two places or more is a
//! count, a cell holding one is the place, and either stands at the center
//! of the z16 ground cell its places' centroid falls in — the page's, or the
//! centroids of the finer cells counted into it weighted by their counts. It
//! stands in all three axes, on the surface the model says is drawn there:
//! what a ray down through the cell meets first, ground or a placed object,
//! at the level the view draws that ground at (world/access.rs `first_hit`)
//! — the world as it is known, never the state of a mesh being built.

use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::enabled;
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
use pxl_spec::space::pyramid::{Cell, LevelPage, GROUND};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    /// What `kept` has on the sheet, in the order it is drawn.
    drawn: Vec<u32>,
    /// The badges drawn, as a tap finds them: nearest the eye first, each by
    /// its box on the screen, what it says and where it stands: the point
    /// it is drawn over, on the surface there.
    hits: Vec<([f64; 4], Label, Pvf3)>,
    /// What a turn works in, kept so a settled turn asks the heap for
    /// nothing: each badge's label and where it stands, and the instances
    /// written for the set.
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
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: high - low,
    };
    let hit = Access::now().first_hit(&ray, Detail::Level(level)).value().flatten()?;
    Some(high - hit.t)
}

/// How many places a badge's cell holds, where it holds one, that place —
/// its page and pool index, read only once the count is known to be one —
/// and the sum of its places' z16 ground cells, as offsets in the badge's
/// cell, which over the count is their centroid.
#[derive(Clone, Copy, Default)]
struct Tally {
    count: u32,
    /// The level the view draws the ground its places stand on at.
    level: u32,
    place: Option<(LevelPage, u32)>,
    at: [u64; 2],
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
    let mut cells: BTreeMap<Cell, Tally> = BTreeMap::new();
    let Some(domain) = crate::world::pyramid::domain::current() else { return };
    for &chunk in &crate::world::pyramid::cover::cover::<PoiPage>().0.drawn {
        // The page that draws the chunk's ground: its own, or where the tree
        // stops above it, the page over it that says that ground — whose cells
        // there hold one place or none, each standing where it stands.
        let Some(src) = pages.source(chunk.page()) else { continue };
        let Some(poi) = pages.get(src) else { continue };
        // The level the view wants the ground at: a finer page standing in
        // counts into its cells, and a coarser one is counted where its places
        // stand, which is its own cells where it is the chunk's page.
        let level = domain.wanted_over(chunk).level();
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
                let ground = cell.inner_at(GROUND, 0, c.at);
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
                let up = POI_LATTICE.cell_of(level, &ground.mid());
                let at = up.offset_of(ground);
                let t = cells.entry(up).or_default();
                t.count += c.count;
                t.level = chunk.level();
                t.place = poi.place(c).map(|_| (src, c.place));
                for (sum, lane) in t.at.iter_mut().zip(at) {
                    *sum += lane as u64 * c.count as u64;
                }
            }
        }
    }
    p.batch.reset();
    let mut labeled = core::mem::take(&mut p.labeled);
    labeled.clear();
    for (cell, t) in cells {
        let label = match (t.count, t.place) {
            (1, Some((page, place))) => Label::Place { page, place },
            _ => Label::Count(t.count),
        };
        let mean = |sum: u64| ((sum + t.count as u64 / 2) / t.count as u64) as u32;
        let stands = cell.inner_at(GROUND, 0, t.at.map(mean));
        let Some(top) = top_at(&stands.mid(), t.level) else { continue };
        let Some(width) = badge::width(label) else { continue };
        let center = stands.center(top);
        let local = crate::world::frame::coord::canonical_to_local(&center, origin);
        p.batch.push(local[0], local[1], local[2], width, badge::ANCHOR, labeled.len() as f64);
        labeled.push((label, center));
    }
    badge::retain(&labeled.iter().map(|l| l.0).collect());
    let v = view();
    select_visible_billboards(&mut p.batch, v.width, v.height, &mut p.kept);
    p.drawn.clear();
    for &i in &p.kept {
        if badge::slot(labeled[i as usize].0) == NO_SLOT {
            continue;
        }
        p.drawn.push(i);
    }
    // Farthest from the eye first. The pass reads the world's depth and
    // writes none of its own, so where two badges cross, the one drawn later
    // is on top, and that has to be the nearer.
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

/// A tap at `ndc_x`, `ndc_y` (in [−1, 1], y up) on the badges: the nearest
/// one whose box it falls in takes it, and the view goes to where that badge
/// stands — a place to the place, a crowd a level closer too, where the
/// ladder draws that ground a level finer and it comes apart into the places
/// under it. Whether a badge took the tap.
pub fn tap(ndc_x: f64, ndc_y: f64) -> bool {
    let v = view();
    let (x, y) = ((ndc_x * 0.5 + 0.5) * v.width, (-ndc_y * 0.5 + 0.5) * v.height);
    let hits = &PLACEMENT.get(Placement::default).hits;
    let Some(&(_, label, stands)) = hits.iter().find(|(r, _, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y)) else {
        return false;
    };
    match label {
        // A count opens up as it is looked at closer.
        Label::Count(_) => crate::control::camera::controller::focus(stands),
        _ => crate::shell::shell::go_to(stands),
    }
    true
}
