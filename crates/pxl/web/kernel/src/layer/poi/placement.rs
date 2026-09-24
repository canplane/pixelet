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
use crate::layer::poi::{badge, enabled};
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::view;
use crate::world::pyramid::Layer;
use pxl_render::asset::atlas::NO_SLOT;

use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::pyramid::{cell_index, Cell, CHUNK_CELLS};
use std::collections::BTreeMap;

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    visible: u32,
}
static PLACEMENT: Global<Placement> = Global::new();

pub fn visible() -> u32 {
    PLACEMENT.get(Placement::default).visible
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if !enabled() || map_store().map_center.is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.visible = 0;
        badge::retain(&[]);
        return;
    }
    let origin = local_world_origin();
    let base = [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))];
    let pages = PoiPage::pyramid();
    let sched = &crate::kernel::kernel().sched;
    let mut cells: BTreeMap<Cell, u32> = BTreeMap::new();
    for &(chunk, _) in &pages.cover().drawn {
        let Some(poi) = pages.get(chunk.page) else { continue };
        for ty in chunk.by..chunk.by + CHUNK_CELLS {
            let (lo, hi) = (cell_index(chunk.bx, ty), cell_index(chunk.bx + CHUNK_CELLS, ty));
            let from = poi.cells.partition_point(|c| c.0 < lo);
            for &(index, n) in poi.cells[from..].iter().take_while(|c| c.0 < hi) {
                let cell = Cell::at(chunk.page, index);
                // Where no ground is drawn under it, it is not drawn either.
                let Some((drawn, _)) = sched.surface_at(cell.mid()) else { continue };
                *cells.entry(cell.up(drawn.saturating_sub(cell.level()))).or_default() += n;
            }
        }
    }
    p.batch.reset();
    let mut counts = Vec::new();
    for (cell, n) in cells {
        let Some((_, top)) = sched.surface_at(cell.mid()) else { continue };
        let centre = cell.centre(top);
        let mut local = [0.0; 3];
        projection::unproject(&mut local, ivec::sub(centre.i, origin), centre.f);
        p.batch
            .push(local[0], local[1], local[2], badge::width(n), badge::COLLIDE, badge::ANCHOR, cell.side(), counts.len() as f64);
        counts.push(n);
    }
    badge::retain(&counts);
    let v = view();
    select_visible_billboards(&mut p.batch, v.width, v.height, &mut p.kept);
    let mut instances = Vec::with_capacity(p.kept.len() * BILLBOARD_FLOATS);
    for &i in &p.kept {
        let i = i as usize;
        let slot = badge::slot(counts[i]);
        if slot == NO_SLOT {
            continue;
        }
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
    p.visible = (instances.len() / BILLBOARD_FLOATS) as u32;
    if p.set.is_none() && instances.is_empty() {
        return;
    }
    let set = *p.set.get_or_insert_with(|| new_billboard_set(BILLBOARD_ORDER::poi));
    place_billboard_set(set, base);
    write_billboard_set(set, &instances);
    ctx.invalidate();
}
