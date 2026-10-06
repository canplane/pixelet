// pxl-render/gpu/surface.rs

//! The canvases the kernel draws into. The page scales whatever buffer exists
//! into the layout box, so a stale or smaller buffer still shows whole.

use xpute_core::math::scalar::round;
use xpute_runtime::abi::handle::{handle_slot, HandleTable};
use xpute_runtime::global::Global;

use crate::gpu::api_spec::surface_role;

/// Slot 0 is never a surface.
pub const SURFACES: usize = 9;

/// Past two, texels cost more than the eye resolves at arm's length. Badge
/// sheets are drawn at DPR_MAX, so a label is never shown smaller than drawn.
pub const DPR_MIN: f64 = 1.0;
pub const DPR_MAX: f64 = 2.0;

/// The picker shows a case at thumbnail size.
const CASE_LONG_SIDE_MAX: f64 = 256.0;

/// A box animating in passes through a size a frame, each of which would
/// otherwise be a new depth and color target.
pub const SETTLE_TURNS: u32 = 4;

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    pub role: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    pub wanted: (u32, u32),
    pub waited: u32,
    pub depth: u32,
    pub depth_view: u32,
}

impl Surface {
    pub fn buffer_px(&self, ratio: Option<f64>) -> (u32, u32) {
        let (w, h) = (self.px_w.max(1) as f64, self.px_h.max(1) as f64);
        let dpr = if self.dpr > 0.0 { self.dpr } else { 1.0 };
        let mut k = ratio.unwrap_or(dpr).clamp(DPR_MIN, DPR_MAX) / dpr;
        if self.role == surface_role::CASE {
            k = k.min(CASE_LONG_SIDE_MAX / w.max(h));
        }
        (round(w * k).max(1.0) as u32, round(h * k).max(1.0) as u32)
    }
}

struct Table {
    handles: HandleTable<SURFACES>,
    surfaces: [Surface; SURFACES],
}

static TABLE: Global<Table> = Global::new();

fn table() -> &'static mut Table {
    TABLE.get(|| Table {
        handles: HandleTable::new(),
        surfaces: [Surface::default(); SURFACES],
    })
}

/// 0 when every surface is taken.
pub fn surface_open(role: u32) -> u32 {
    let t = table();
    let Some(handle) = t.handles.acquire() else { return 0 };
    t.surfaces[handle_slot(handle) as usize] = Surface {
        handle,
        role,
        dpr: 1.0,
        ..Surface::default()
    };
    handle
}

/// Whether the handle was live.
pub fn surface_resize(handle: u32, px_w: u32, px_h: u32, dpr: f64) -> bool {
    let Some(s) = surface(handle) else { return false };
    s.px_w = px_w;
    s.px_h = px_h;
    s.dpr = dpr;
    true
}

pub fn surface_close(handle: u32) -> Option<Surface> {
    let t = table();
    if !t.handles.release(handle) {
        return None;
    }
    let slot = handle_slot(handle) as usize;
    let closed = t.surfaces[slot];
    t.surfaces[slot] = Surface::default();
    Some(closed)
}

pub fn surface(handle: u32) -> Option<&'static mut Surface> {
    let t = table();
    t.handles.live(handle).then(|| &mut t.surfaces[handle_slot(handle) as usize])
}

pub fn surfaces() -> impl Iterator<Item = &'static mut Surface> {
    table().surfaces.iter_mut().filter(|s| s.handle != 0)
}

pub fn surfaces_lost() {
    for s in surfaces() {
        *s = Surface {
            handle: s.handle,
            role: s.role,
            px_w: s.px_w,
            px_h: s.px_h,
            dpr: s.dpr,
            ..Surface::default()
        };
    }
}
