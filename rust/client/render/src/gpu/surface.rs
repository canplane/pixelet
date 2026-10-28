// pxl-render/gpu/surface.rs

//! The canvases the kernel draws into. The page scales whatever buffer exists
//! into the layout box, so a stale or smaller buffer still shows whole.

use xpute_guest::abi::handle::{handle_slot, Table as Handles};
use xpute_guest::global::Global;
use xpute_kit::math::scalar::round;

use crate::gpu::api::Format;

/// Slot 0 is never a surface.
pub const SURFACES: usize = 9;

/// A box animating in passes through a size a frame, each of which would
/// otherwise be a new depth and color target.
pub const SETTLE_TURNS: u32 = 4;

/// The world's color as its passes leave it, toned and linear, blended there
/// and encoded onto the canvas once (gpu/finish.rs): a float, so the finish's
/// dither falls between levels the canvas has and none the color had.
pub const WORLD_COLOR: Format = Format::rgba16float;

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    pub role: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    /// The device pixels per CSS pixel the kernel has it drawn at (pxl-kernel
    /// domain/view/frame/resolution.rs): the display's, or under load a share of
    /// them, the page scaling the buffer up.
    pub ratio: f64,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    pub wanted: (u32, u32),
    pub waited: u32,
    /// The frame's depth, the world's and over it what moves on its own
    /// (gpu/renderer.rs, stage 2).
    pub depth: u32,
    pub depth_view: u32,
    /// The color its passes draw into (`WORLD_COLOR`), made with the depth,
    /// which the finish encodes onto the canvas.
    pub color: u32,
    pub color_view: u32,
    /// The eye's rays as the cast leaves them, a drawn pixel each, made with
    /// the depth: `ray::WORDS` layers of `r32uint`, a word of the ray's a
    /// layer (pxl-spec-core `ray`).
    pub rays: u32,
    pub rays_view: u32,
}

impl Surface {
    /// What it holds on the GPU, every one made again with its size.
    pub fn targets(&self) -> [u32; 6] {
        [
            self.depth_view,
            self.depth,
            self.color_view,
            self.color,
            self.rays_view,
            self.rays,
        ]
    }

    pub fn buffer_px(&self) -> (u32, u32) {
        let (w, h) = (self.px_w.max(1) as f64, self.px_h.max(1) as f64);
        let dpr = if self.dpr > 0.0 { self.dpr } else { 1.0 };
        let k = self.ratio / dpr;
        (round(w * k).max(1.0) as u32, round(h * k).max(1.0) as u32)
    }
}

struct Table {
    handles: Handles<(), SURFACES>,
    surfaces: [Surface; SURFACES],
}

static TABLE: Global<Table> = Global::new();

fn table() -> &'static mut Table {
    TABLE.get(|| Table {
        handles: Handles::new(),
        surfaces: [Surface::default(); SURFACES],
    })
}

/// 0 when every surface is taken.
pub fn surface_open(role: u32) -> u32 {
    let t = table();
    let Some(handle) = t.handles.insert(()) else {
        return 0;
    };
    t.surfaces[handle_slot(handle) as usize] = Surface {
        handle,
        role,
        dpr: 1.0,
        ratio: 1.0,
        ..Surface::default()
    };
    handle
}

/// Whether the handle was live.
pub fn surface_resize(handle: u32, px_w: u32, px_h: u32, dpr: f64, ratio: f64) -> bool {
    let Some(s) = surface(handle) else {
        return false;
    };
    s.px_w = px_w;
    s.px_h = px_h;
    s.dpr = dpr;
    s.ratio = ratio;
    true
}

pub fn surface_close(handle: u32) -> Option<Surface> {
    let t = table();
    t.handles.remove(handle)?;
    let slot = handle_slot(handle) as usize;
    let closed = t.surfaces[slot];
    t.surfaces[slot] = Surface::default();
    Some(closed)
}

pub fn surface(handle: u32) -> Option<&'static mut Surface> {
    let t = table();
    t.handles
        .live(handle)
        .then(|| &mut t.surfaces[handle_slot(handle) as usize])
}

pub fn surfaces() -> impl Iterator<Item = &'static mut Surface> {
    table().surfaces.iter_mut().filter(|s| s.handle != 0)
}

pub fn on_surfaces_lost() {
    for s in surfaces() {
        *s = Surface {
            handle: s.handle,
            role: s.role,
            px_w: s.px_w,
            px_h: s.px_h,
            dpr: s.dpr,
            ratio: s.ratio,
            ..Surface::default()
        };
    }
}
