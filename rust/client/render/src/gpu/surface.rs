// pxl-render/gpu/surface.rs

//! The canvases the kernel draws into, each at its display's device pixels,
//! the world inside it at its own (`render_px`). The page scales whatever
//! buffer exists into the layout box, so a stale one still shows whole.

use xpute_core::abi::handle::{handle_slot, Table as Handles};
use xpute_core::global::Global;
use xpute_kit::math::scalar::round;

use crate::api_spec::surface_role;
use crate::frame::shader::word::screen::TILE;
use crate::gpu::api::Format;
use crate::gpu::ray::{drawn_fits, DRAWN_AREA, DRAWN_MOST};
use pxl_engine::WORKGROUPS_MOST;

/// Slot 0 is never a surface.
pub const SURFACES: usize = 9;

/// A box animating in passes through a size a frame, each of which would
/// otherwise be a new depth and color target.
pub const SETTLE_TURNS: u32 = 4;

/// The world's color as its passes leave it, toned and linear, blended
/// there, gathered over frames (gpu/resolve.rs) and encoded onto the canvas
/// once (gpu/finish.rs): a float, so the finish's dither falls between
/// levels the canvas has and none the color had.
pub const WORLD_COLOR: Format = Format::rgba16float;

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    /// What it draws, a `surface_role`.
    pub role: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    /// The device pixels per CSS pixel the kernel has it drawn at (pxl-kernel
    /// domain/scene/resolution.rs): the display's, or under load a share of
    /// them, the finish bringing them up to the canvas's (`render_px`).
    pub ratio: f64,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    /// The world's size and the canvas's last asked, which hold
    /// `SETTLE_TURNS` before the targets are made at them.
    pub wanted_sizes: ((u32, u32), (u32, u32)),
    pub waited_turns: u32,
    /// The frame's depth, the world's and over it what moves on its own
    /// (gpu/renderer.rs, stage 2), with a stencil (`GATED_DEPTH`):
    /// `depth_view` as a pass draws into it,
    /// `depth_only_view` as a binding reads it.
    pub depth: u32,
    pub depth_view: u32,
    pub depth_only_view: u32,
    /// The color its passes draw into (`WORLD_COLOR`), made with the depth,
    /// which the world's gathers over frames (`history`) and the finish
    /// encodes onto the canvas.
    pub color: u32,
    pub color_view: u32,
    /// The canvas's size, its device pixels (`screen_px`), and the depth
    /// at it the finish carries the world's to (gpu/finish.rs), which what
    /// is drawn over the world is tested against.
    pub screen: (u32, u32),
    pub screen_depth: u32,
    pub screen_depth_view: u32,
    /// The eye's rays as the cast leaves them, a drawn pixel each, made with
    /// the depth: `ray::WORDS` layers of `r32uint`, a word of the ray's a
    /// layer (pxl-engine `ray`).
    pub rays: u32,
    pub rays_view: u32,
    /// The world's color gathered over frames, two of `WORLD_COLOR` made
    /// with the depth, the last frame's read as this one's is written, in
    /// turn (gpu/resolve.rs): the world's alone.
    pub history: [u32; 2],
    pub history_view: [u32; 2],
    /// The finished picture at the canvas's size, the canvas's format, made
    /// with the screen's depth: every frame finishes into it in the pass that
    /// finishes the canvas, a gated frame only its claimed tiles, once it
    /// has shown it whole on the canvas, whose texture is a new one each
    /// frame (gpu/finish.rs): the world's alone.
    pub picture: u32,
    pub picture_view: u32,
}

impl Surface {
    /// What it holds on the GPU, every one made again with its size.
    pub fn targets(&self) -> [u32; 15] {
        [
            self.depth_only_view,
            self.depth_view,
            self.depth,
            self.color_view,
            self.color,
            self.screen_depth_view,
            self.screen_depth,
            self.rays_view,
            self.rays,
            self.history_view[0],
            self.history[0],
            self.history_view[1],
            self.history[1],
            self.picture_view,
            self.picture,
        ]
    }

    /// The canvas's device pixels: the display's, whatever the world is drawn
    /// at.
    pub fn screen_px(&self) -> (u32, u32) {
        (self.px_w.max(1), self.px_h.max(1))
    }

    /// The drawn pixels a CSS pixel spans across.
    pub fn px_per_css(&self) -> f64 {
        f64::from(self.width) * self.dpr / f64::from(self.px_w.max(1))
    }

    /// The world's pixels: the canvas's at the kernel's share of them
    /// (`ratio` over the display's), which the finish brings up to the
    /// canvas, and no more than every device draws (ray/cast.rs
    /// `drawn_fits`), the canvas's shape kept: its share brought down until
    /// they fit.
    pub fn render_px(&self) -> (u32, u32) {
        let (w, h) = (self.px_w.max(1) as f64, self.px_h.max(1) as f64);
        let dpr = if self.dpr > 0.0 { self.dpr } else { 1.0 };
        let most = f64::from(DRAWN_MOST[0].min(DRAWN_MOST[1]));
        let mut k = (self.ratio / dpr).min(most / w.max(h));
        loop {
            let size = [w, h].map(|v| round(v * k).clamp(1.0, most) as u32);
            if drawn_fits(size) {
                return (size[0], size[1]);
            }
            // The pixels or the tiles past their most: the share a side brought down by the root
            // of the tighter, and a step more, so each turn takes some off.
            let tiles = [0, 1].map(|a| f64::from(size[a].div_ceil(TILE)));
            let limit = f64::from(WORKGROUPS_MOST) / (tiles[0] * tiles[1]);
            let area = f64::from(DRAWN_AREA) / (f64::from(size[0]) * f64::from(size[1]));
            k *= libm::sqrt(limit.min(area)) * (1.0 - 1.0 / most);
        }
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

/// A canvas that draws nothing until its role is set (`surface_role`); 0
/// when every surface is taken.
pub fn surface_open() -> u32 {
    let t = table();
    let Some(handle) = t.handles.insert(()) else {
        return 0;
    };
    t.surfaces[handle_slot(handle) as usize] = Surface {
        handle,
        role: surface_role::NONE,
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

#[cfg(test)]
#[path = "surface.test.rs"]
mod test;
