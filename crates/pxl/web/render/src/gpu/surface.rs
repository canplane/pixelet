// pxl-render/gpu/surface.rs

//! The canvases the kernel draws into, each named by the handle it gave the
//! shell when the shell handed one over: the world's, and a case's for the
//! variant picker. The shell mounts a canvas and says how many device pixels
//! its layout box is and at what pixel ratio; the buffer it is drawn into, and
//! what is drawn, are the kernel's. The page scales whatever buffer there is
//! into the box, so a buffer smaller than the box, or one made for a size the
//! box is only passing through, still shows whole and in proportion.

use xpute_core::math::scalar::round;
use xpute_runtime::abi::handle::{handle_slot, HandleTable};
use xpute_runtime::global::Global;

use crate::gpu::api_spec::surface_role;

/// Surfaces open at once, and slot 0, which is never one.
pub const SURFACES: usize = 9;

/// The device pixels a CSS pixel is drawn at: the device's, clamped. Past two
/// the texels cost more than the eye resolves at arm's length; under one is a
/// zoomed-out page, drawn at least at its own pixels. The billboards' sheets
/// are drawn at DPR_MAX, so a label is never shown smaller than it was drawn.
/// The world's surface is drawn at what the kernel's resolution chooses
/// inside these (pxl-kernel world/frame/resolution.rs).
pub const DPR_MIN: f64 = 1.0;
pub const DPR_MAX: f64 = 2.0;

/// A case's buffer on its long side, at most: the picker shows it at a
/// thumbnail's size, and a buffer past that only costs.
const CASE_LONG_SIDE_MAX: f64 = 256.0;

/// Turns a new buffer size must hold before the buffer is made again: a box
/// that is animating in passes through a size a frame, and each would be a new
/// depth and colour target.
pub const SETTLE_TURNS: u32 = 4;

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    pub role: u32,
    /// The layout box in device pixels, and the device's pixel ratio.
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    /// Its context configured on the device the renderer has.
    pub configured: bool,
    /// The buffer its targets were made at.
    pub width: u32,
    pub height: u32,
    /// A size the buffer is waiting to be made at, and the turns it has held.
    pub wanted: (u32, u32),
    pub waited: u32,
    pub depth: u32,
    pub depth_view: u32,
}

impl Surface {
    /// The buffer its box is drawn into: the box's device pixels at `ratio`
    /// to a CSS pixel, or where there is none, the device's at no more than
    /// DPR_MAX; a case's no longer than CASE_LONG_SIDE_MAX, both sides scaled
    /// alike.
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

/// A handle for a canvas the shell hands over to show `role`; 0 when every
/// surface is taken. Whoever hands it over asks for the turn that draws it.
pub fn surface_create(role: u32) -> u32 {
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

/// The canvas's layout box in device pixels, and the device's pixel ratio.
/// Whether the handle was live: the caller asks for the turn that redraws it.
pub fn surface_update(handle: u32, px_w: u32, px_h: u32, dpr: f64) -> bool {
    let Some(s) = surface(handle) else { return false };
    s.px_w = px_w;
    s.px_h = px_h;
    s.dpr = dpr;
    true
}

/// Closes a surface; what it held on the device is let go (renderer.rs).
pub fn surface_delete(handle: u32) -> Option<Surface> {
    let t = table();
    if !t.handles.release(handle) {
        return None;
    }
    let slot = handle_slot(handle) as usize;
    let closed = t.surfaces[slot];
    t.surfaces[slot] = Surface::default();
    Some(closed)
}

/// The surface a live handle names.
pub fn surface(handle: u32) -> Option<&'static mut Surface> {
    let t = table();
    t.handles.live(handle).then(|| &mut t.surfaces[handle_slot(handle) as usize])
}

/// Every open surface.
pub fn surfaces() -> impl Iterator<Item = &'static mut Surface> {
    table().surfaces.iter_mut().filter(|s| s.handle != 0)
}

/// The device went away, and what every surface held on it with it.
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
