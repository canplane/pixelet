// pxl-kernel/layer/poi/placement.rs

use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::frustum_budget::select_visible_billboards;
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BILLBOARD_ORDER};
use crate::layer::poi::{
    badge,
    store::{self, CANDIDATES, RESIDENT_BYTES},
};
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::view;
use pxl_render::asset::atlas::NO_SLOT;
use pxl_spec::poi::Summary;
use pxl_spec::space::pvf3::{ivec, projection, Pvf3};

const VISIBLE: usize = 4;

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    set: Option<u32>,
    visible: u32,
}
static PLACEMENT: Global<Placement> = Global::new();

/// A summary's midpoint snapped to the displayed terrain cell and surface.
/// This presentation position never enters canonical storage or node packets.
pub fn anchor(summary: &Summary) -> Option<[f64; 3]> {
    let b = summary.bounds;
    crate::kernel::kernel().sched.surface_anchor((b[0] + b[3]) * 0.5, (b[1] + b[4]) * 0.5)
}

pub fn visible() -> u32 {
    PLACEMENT.get(Placement::default).visible
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if !store::enabled() || map_store().map_center.is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.visible = 0;
        badge::retain(&[]);
        *store::store() = store::Store::default();
        store::demand();
        return;
    }
    let origin = local_world_origin();
    let base = [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))];
    let v = view();
    let eye = [base[0] + v.eye_local.x, base[1] + v.eye_local.y, base[2] + v.eye_local.z];
    let s = store::store();
    s.select(eye, CANDIDATES, RESIDENT_BYTES);
    store::demand();
    let cut = &s.cut;
    badge::retain(&cut.iter().map(|s| s.count).collect::<Vec<_>>());
    p.batch.reset();
    let mut grounded = Vec::new();
    for summary in cut {
        let Some(a) = anchor(summary) else { continue };
        grounded.push(summary);
        let mut absolute = Pvf3::default();
        projection::project(&mut absolute, a[0], a[1], a[2]);
        let mut local = [0.0; 3];
        projection::unproject(&mut local, ivec::sub(absolute.i, origin), absolute.f);
        p.batch
            .push(local[0], local[1], local[2], badge::width(summary.count), badge::COLLIDE, badge::ANCHOR, summary.key as f64);
    }
    let mut kept = [0u32; VISIBLE];
    let n = select_visible_billboards(&mut p.batch, VISIBLE as u32, v.width, v.height, &mut kept);
    let mut instances = Vec::with_capacity(CANDIDATES * 10);
    for &i in &kept[..n as usize] {
        let i = i as usize;
        let slot = badge::slot(grounded[i].count);
        if slot == NO_SLOT {
            continue;
        }
        let sheet = billboard_sheet();
        let slot = slot as u32;
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
    p.visible = (instances.len() / 10) as u32;
    if p.set.is_none() && instances.is_empty() {
        return;
    }
    let set = *p.set.get_or_insert_with(|| new_billboard_set(BILLBOARD_ORDER::poi));
    place_billboard_set(set, base);
    write_billboard_set(set, &instances);
    ctx.invalidate();
}
