// pxl-kernel/layer/poi/placement.rs

//! Where the counts are drawn. What the POI pages draw of the view is the
//! layer's own cover (world/pyramid.rs `Cover`): pieces of the ground in
//! view, none over another, so every place under them is counted once. Each
//! of their cells — cells of the POI lattice (pxl-spec bake/poi_page.rs
//! `POI_LATTICE`) — is counted into itself, or into the cell over it at the
//! level the view wants there, where a finer page stands in for it: how
//! coarse places are is the camera's distance and nothing else, the ground's
//! the same way but apart from it. A cell holding two places or more is a
//! count at that cell's centre; a cell holding one is the place, at the
//! centre of the level-0 ground cell it stands in. Either stands in all
//! three axes, on the surface drawn there (pxl-spec space/pyramid.rs
//! `Cell::centre`, chunk/table.rs `surface_at`), which is the one thing read
//! from the ground.

use crate::abi::cmd::PoiBadge;
use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::field::BILLBOARD_FLOATS;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::enabled;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::view;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;

use pxl_spec::bake::poi_page::{PoiPage, POI_LATTICE};
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::pyramid::{cell_base, Cell, LevelPage, GROUND, TOP_LEVEL};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    /// The badges shown, nearest what the view looks at first: what the
    /// shell lists beside the map (`snapshot`).
    shown: Vec<(Cell, Label, u32)>,
}
static PLACEMENT: Global<Placement> = Global::new();

/// How many places a badge's cell holds, and where it holds one, that
/// place's name — its page and pool index, read only once the count is known
/// to be one — and the level-0 ground cell it stands in.
#[derive(Clone, Copy, Default)]
struct Tally {
    count: u32,
    name: Option<(LevelPage, u32)>,
    at: Option<Cell>,
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if !enabled() || map_store().map_center.is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.shown.clear();
        badge::retain(&HashSet::new());
        return;
    }
    let origin = local_world_origin();
    let base = [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))];
    let pages = PoiPage::pyramid();
    let sched = &crate::kernel::kernel().sched;
    let mut cells: BTreeMap<Cell, Tally> = BTreeMap::new();
    let (n, across) = (POI_LATTICE.chunk_cells(), POI_LATTICE.page_cells());
    let Some(domain) = pages.domain() else { return };
    for &chunk in &pages.cover().0.drawn {
        let Some(poi) = pages.get(chunk.page) else { continue };
        // A finer page standing in counts into the cells the view wants; a
        // coarser one is as fine as there is until the view's own arrives.
        let coarser = domain.wanted_over(chunk).level() - chunk.level();
        // The chunk's ground, in the POI lattice's cells of its level.
        let first = POI_LATTICE.cell_of(chunk.level(), &cell_base(chunk.page, chunk.bx, chunk.by)).index();
        let (x0, y0) = (first % across, first / across);
        for ty in y0..y0 + n {
            let (lo, hi) = (POI_LATTICE.cell_index(x0, ty), POI_LATTICE.cell_index(x0 + n, ty));
            let from = poi.cells.partition_point(|c| c.cell < lo);
            for c in poi.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = POI_LATTICE.cell_at(chunk.page, c.cell);
                let t = cells.entry(cell.up(coarser)).or_default();
                t.count += c.count;
                t.name = poi.name(c).map(|_| (chunk.page, c.name));
                t.at = c.at.map(|at| cell.inner_at(GROUND, 0, at));
            }
        }
    }
    p.batch.reset();
    let mut labelled = Vec::new();
    for (cell, t) in cells {
        let (label, stands) = match (t.count, t.at) {
            (1, Some(at)) => (t.name.map_or(Label::Count(1), |(page, name)| Label::Place { page, name }), at),
            _ => (Label::Count(t.count), cell),
        };
        let Some((_, top)) = sched.surface_at(&stands.mid()) else { continue };
        let centre = stands.centre(top);
        let mut local = [0.0; 3];
        projection::unproject(&mut local, ivec::sub(centre.i, origin), centre.f);
        p.batch.push(local[0], local[1], local[2], badge::width(label), badge::ANCHOR, cell.side(), labelled.len() as f64);
        labelled.push((cell, label, t.count));
    }
    badge::retain(&labelled.iter().map(|l| l.1).collect());
    let v = view();
    select_visible_billboards(&mut p.batch, v.width, v.height, &mut p.kept);
    let mut instances = Vec::with_capacity(p.kept.len() * BILLBOARD_FLOATS);
    p.shown.clear();
    for &i in &p.kept {
        let i = i as usize;
        let slot = badge::slot(labelled[i].1);
        if slot == NO_SLOT {
            continue;
        }
        p.shown.push(labelled[i]);
        let (sheet, slot) = (billboard_sheet(), slot as u32);
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
            p.batch.unit[i] as f32,
        ]);
    }
    if p.set.is_none() && instances.is_empty() {
        return;
    }
    let set = *p.set.get_or_insert_with(|| new_billboard_set(BillboardOrder::Poi));
    place_billboard_set(set, base);
    write_billboard_set(set, &instances);
    ctx.invalidate();
}

/// The badges shown, as the shell lists them: nearest what the view looks at
/// first, each by its cell — which the shell hands back to go there
/// (`focus`) — its count, and the name where it is one place.
pub fn badges(each: &mut dyn FnMut(&PoiBadge<'_>)) {
    for &(cell, label, count) in PLACEMENT.get(Placement::default).shown.iter() {
        let text = match label {
            Label::Place { .. } => Some(label.text()),
            Label::Count(_) => None,
        };
        each(&PoiBadge {
            page: cell.page().subject(),
            index: cell.index(),
            count,
            name: text.as_ref().map(|t| t as &dyn core::fmt::Display),
        });
    }
}

/// Goes to the cell a badge stands for, by what `badges` gave it: its
/// middle, a level closer, where the ladder draws that ground a level finer
/// and the cell comes apart into the places under it. Whether the two named
/// a cell.
pub fn focus(subject: u64, index: u32) -> bool {
    // A cell from outside: a level past the pyramid, a subject that does not
    // give itself back, or an index past the page is no cell.
    let page = LevelPage::of_subject(subject);
    let across = POI_LATTICE.page_cells();
    if page.level > TOP_LEVEL || page.subject() != subject || index >= across * across {
        return false;
    }
    let cell = POI_LATTICE.cell_at(page, index);
    crate::shell::shell::go_to(cell.mid());
    let camera = crate::control::camera::store::camera_store();
    let dist = Some(camera.view_state.dist / 2.0);
    camera.set_view_state(crate::control::camera::store::ViewPatch { dist, azimuth: None, polar: None });
    true
}
