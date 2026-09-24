// pxl-kernel/layer/poi/placement.rs

//! Where the counts are drawn. A chunk drawn at level L with step s shows its
//! ground in blocks of s cells of that level, so its places are counted in
//! the same blocks and stood at the same centre (pxl-spec space/pyramid.rs
//! `cell_centre`), on the top of the surface drawn there. Nothing here decides
//! a level or a resolution of its own: a place is shown as coarsely as the
//! ground under it is.

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
use pxl_spec::space::pyramid::CHUNK_CELLS;

use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pvf3::{ivec, projection, Pvf3};
use pxl_spec::space::pyramid::{cell_centre, cell_index, cell_of, PAGE_CELLS};

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
    p.batch.reset();
    let mut counts = Vec::new();
    for d in crate::kernel::kernel().sched.drawn() {
        let (page, first) = cell_of(d.level, d.base);
        let Some(poi) = pages.get(page) else { continue };
        let (x0, y0) = (first % PAGE_CELLS, first / PAGE_CELLS);
        let axis = CHUNK_CELLS / d.step;
        let mut blocks: Vec<(u32, u32)> = Vec::new();
        for ty in y0..y0 + CHUNK_CELLS {
            let (lo, hi) = (cell_index(x0, ty), cell_index(x0 + CHUNK_CELLS, ty));
            let from = poi.cells.partition_point(|c| c.0 < lo);
            for &(cell, n) in poi.cells[from..].iter().take_while(|c| c.0 < hi) {
                let block = ((ty - y0) / d.step) * axis + (cell % PAGE_CELLS - x0) / d.step;
                match blocks.iter_mut().find(|b| b.0 == block) {
                    Some(b) => b.1 += n,
                    None => blocks.push((block, n)),
                }
            }
        }
        for (block, n) in blocks {
            let (bx, by) = (block % axis, block / axis);
            let Some(z) = d.top(bx, by) else { continue };
            let centre = cell_centre(page, cell_index(x0 + bx * d.step, y0 + by * d.step), d.step);
            let mut at = [0.0; 3];
            projection::unproject(&mut at, centre.i, centre.f);
            let mut absolute = Pvf3::default();
            projection::project(&mut absolute, at[0], at[1], z);
            let mut local = [0.0; 3];
            projection::unproject(&mut local, ivec::sub(absolute.i, origin), absolute.f);
            p.batch.push(
                local[0],
                local[1],
                local[2],
                badge::width(n),
                badge::COLLIDE,
                badge::ANCHOR,
                f64::from(d.step << d.level),
                counts.len() as f64,
            );
            counts.push(n);
        }
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
