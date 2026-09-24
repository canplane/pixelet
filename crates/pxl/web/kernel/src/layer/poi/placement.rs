// pxl-kernel/layer/poi/placement.rs

//! Where the counts are drawn. What the POI pages draw of the view is the
//! layer's own cover (world/pyramid.rs `Cover`): pieces of the ground in
//! view, none over another, so every place under them is counted once. Each
//! of their cells is counted into the cell the ground is drawn in there — a
//! cell of the level the terrain's chunk draws blocks at, or the POI cell
//! itself where that is coarser — and the badge stands at that cell's centre
//! in all three axes, on the surface drawn there (pxl-spec
//! space/pyramid.rs `Cell::centre`, chunk/table.rs `surface_at`). Nothing
//! here decides a level or a resolution of its own: a place is shown as
//! coarsely as the ground under it is, or as its own page is where that is
//! all that has come.

use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::field::BILLBOARD_FLOATS;
use crate::layer::billboard::selection::select_visible_billboards;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BILLBOARD_ORDER};
use crate::layer::poi::badge::{self, Label};
use crate::layer::poi::enabled;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::view;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;

use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::pyramid::{cell_index, Cell, LevelPage, CHUNK_CELLS};
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

/// How many places a badge's cell holds, and the one place's name where it
/// holds one: the name's page and pool index, read only once the count is
/// known to be one.
#[derive(Clone, Copy, Default)]
struct Tally {
    count: u32,
    name: Option<(LevelPage, u32)>,
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
    for &chunk in &pages.cover().0.drawn {
        let Some(poi) = pages.get(chunk.page) else { continue };
        for ty in chunk.by..chunk.by + CHUNK_CELLS {
            let (lo, hi) = (cell_index(chunk.bx, ty), cell_index(chunk.bx + CHUNK_CELLS, ty));
            let from = poi.cells.partition_point(|c| c.cell < lo);
            for c in poi.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = Cell::at(chunk.page, c.cell);
                // Where no ground is drawn under it, it is not drawn either.
                let Some((drawn, _)) = sched.surface_at(cell.mid()) else { continue };
                let t = cells.entry(cell.up(drawn.saturating_sub(cell.level()))).or_default();
                t.count += c.count;
                t.name = poi.name(c).map(|_| (chunk.page, c.name));
            }
        }
    }
    p.batch.reset();
    let mut labelled = Vec::new();
    for (cell, t) in cells {
        let Some((_, top)) = sched.surface_at(cell.mid()) else { continue };
        let label = match t.name {
            Some((page, name)) if t.count == 1 => Label::Place { page, name },
            _ => Label::Count(t.count),
        };
        let centre = cell.centre(top);
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
    let set = *p.set.get_or_insert_with(|| new_billboard_set(BILLBOARD_ORDER::poi));
    place_billboard_set(set, base);
    write_billboard_set(set, &instances);
    ctx.invalidate();
}

/// The badges shown, as the shell lists them: nearest what the view looks at
/// first, each by its cell — which the shell hands back to go there
/// (`focus`) — its count, and the name where it is one place.
pub fn snapshot<W: std::fmt::Write>(out: &mut W) -> std::fmt::Result {
    use xpute_core::codec::json::Quoted;
    out.write_char('[')?;
    for (i, &(cell, label, count)) in PLACEMENT.get(Placement::default).shown.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        write!(out, "{{\"id\":{},\"count\":{count},\"name\":", Quoted(format!("{}:{}", cell.page().subject(), cell.index())))?;
        match label {
            Label::Place { .. } => write!(out, "{}}}", Quoted(label.text()))?,
            Label::Count(_) => out.write_str("null}")?,
        }
    }
    out.write_char(']')
}

/// Goes to the cell a badge stands for, by the id `snapshot` gave it: its
/// middle, a level closer, where the ladder draws that ground a level finer
/// and the cell comes apart into the places under it. Whether the id named
/// a cell.
pub fn focus(id: &str) -> bool {
    let Some((subject, index)) = id.split_once(':') else { return false };
    let (Ok(subject), Ok(index)) = (subject.parse::<u64>(), index.parse::<u32>()) else { return false };
    if index >= pxl_spec::space::pyramid::PAGE_CELLS * pxl_spec::space::pyramid::PAGE_CELLS {
        return false;
    }
    let cell = Cell::at(LevelPage::of_subject(subject), index);
    crate::shell::shell::go_to(pxl_spec::space::pvf3::Pvf3 { i: cell.mid(), ..Default::default() });
    let camera = crate::control::camera::store::camera_store();
    let dist = Some(camera.view_state.dist / 2.0);
    camera.set_view_state(crate::control::camera::store::ViewPatch { dist, azimuth: None, polar: None });
    true
}
