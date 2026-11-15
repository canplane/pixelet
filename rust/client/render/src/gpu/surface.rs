// pxl-render/gpu/surface.rs

//! The canvases the core draws into. Each canvas is sized in its display's device pixels, and
//! the world inside it is drawn at a size of its own (`render_px`). The page scales whatever
//! buffer the canvas has into its layout box, so a buffer of a stale size still shows whole.

use pxl_global::Global;
use xpute_kit::math::scalar::round;

use crate::frame::shader::floor;
use crate::frame::shader::word::screen::TILE;
use crate::spec::api::surface_role;
use pxl_engine::{WORKGROUP_CAP, ray};

/// The surface table's slots: at most this many surfaces are open at once.
pub const SURFACE_CAP: usize = 9;

/// The frames a new size must hold before the targets are made at it. A box being animated
/// passes through a new size each frame, and each size would otherwise make new depth and
/// color targets.
pub const SETTLE_FRAME_CNT: u32 = 4;

/// The most pixels the world is drawn at along each side: the largest texture side of the
/// floor, the limits every WebGPU device meets (frame/shader/floor.rs). Every size that
/// `drawn_fits` allows lies within it, and at that size the screen's damaged tiles, a word a
/// tile (ray/cast.rs `CAST_TILES_START`), still fit the floor.
pub const DRAWN_CAP: [u32; 2] = [floor::MAX_TEXTURE_DIMENSION_2D; 2];

/// The most pixels the world is drawn at in all. The eye's holes (`ray::holes_words`) take a
/// word and a bit a pixel, and with the eye's pool they fit within 27/64 of the floor's
/// largest storage binding. The rest of that binding is left for the camera grid's words at
/// the widest field, which lie beside the holes in the beam rays' buffer.
pub const DRAWN_AREA: u32 =
    ((floor::MAX_STORAGE_BUFFER_BINDING_SIZE * 27 / 256) as u32 - ray::POOL_WORDS - 1) / 33 * 32;

/// Whether every device draws the world at `size` pixels (`render_px`). No side may pass
/// `DRAWN_CAP`, and the area may not pass `DRAWN_AREA`. The pixels' cast (cast.wgsl `cast_cs`)
/// dispatches a workgroup for each screen tile along one side of its dispatch, so the tiles
/// may not pass the floor's workgroups a side (`WORKGROUP_CAP`).
pub const fn drawn_fits(size: [u32; 2]) -> bool {
    size[0] <= DRAWN_CAP[0]
        && size[1] <= DRAWN_CAP[1]
        && size[0].div_ceil(TILE) * size[1].div_ceil(TILE) <= WORKGROUP_CAP
        && size[0] * size[1] <= DRAWN_AREA
}

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    /// What it draws, a `surface_role`.
    pub role: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    /// The device pixels per CSS pixel at which the core has the world drawn (pxl-core
    /// domain/scene/resolution.rs). It is the display's ratio, or a share of it under load,
    /// and the finish brings the drawn pixels up to the canvas's (`render_px`).
    pub ratio: f64,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    /// The world's size and the canvas's size as last asked. They must hold for
    /// `SETTLE_FRAME_CNT` frames before the targets are made at them.
    pub wanted_sizes: ((u32, u32), (u32, u32)),
    pub waited_frame_cnt: u32,
    /// The frame's depth, with a stencil (`MASKED_DEPTH`). The world draws into it, and then
    /// what moves on its own draws over the world (renderer.rs, stage 2). A pass draws
    /// into it through `depth_view`, and a binding reads it through `depth_only_view`.
    pub depth: u32,
    pub depth_view: u32,
    pub depth_only_view: u32,
    /// The color the passes draw into (gpu/api.rs `WORLD_COLOR`), made with the depth. The
    /// world's color is gathered over frames (`history`), and the finish encodes it onto the
    /// canvas.
    pub color: u32,
    pub color_view: u32,
    /// The canvas's size in device pixels (`screen_px`), and a depth at that size. The finish
    /// carries the world's depth up into it (pass/finish.rs), and what is drawn over the world
    /// is tested against it.
    pub screen: (u32, u32),
    pub screen_depth: u32,
    pub screen_depth_view: u32,
    /// The eye's rays as the cast leaves them, one for each drawn pixel, made with the depth.
    /// The texture has `ray::WORDS` layers of `r32uint`, and each layer holds one word of each
    /// ray (pxl-engine `ray`).
    pub rays: u32,
    pub rays_view: u32,
    /// The world's color gathered over frames: two textures of `WORLD_COLOR`, made with the
    /// depth. They take turns, the last frame's read while this frame's is written
    /// (pass/resolve.rs). Only a world surface has them.
    pub history: [u32; 2],
    pub history_view: [u32; 2],
    /// The finished picture, at the canvas's size and in the canvas's format, made with the
    /// screen's depth. Every frame finishes into it in the same pass that finishes the canvas.
    /// The canvas's texture is a new one each frame, so a gated frame first shows the picture
    /// whole on the canvas and then finishes both only in its damaged tiles (pass/finish.rs).
    /// Only a world surface has it.
    pub picture: u32,
    pub picture_view: u32,
}

impl Surface {
    /// The GPU objects it holds, all of which are made again when its size changes.
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

    /// The canvas's device pixels: the display's, whatever size the world is drawn at.
    pub fn screen_px(&self) -> (u32, u32) {
        (self.px_w.max(1), self.px_h.max(1))
    }

    /// The drawn pixels a CSS pixel spans across.
    pub fn px_per_css(&self) -> f64 {
        f64::from(self.width) * self.dpr / f64::from(self.px_w.max(1))
    }

    /// The world's size in pixels: the canvas's pixels at the core's share of them (`ratio`
    /// over the display's ratio), which the finish brings up to the canvas. Where that size is
    /// more than every device draws (`drawn_fits`), the share is brought down until it fits,
    /// and the canvas's shape is kept.
    pub fn render_px(&self) -> (u32, u32) {
        let (w, h) = (self.px_w.max(1) as f64, self.px_h.max(1) as f64);
        let dpr = if self.dpr > 0.0 { self.dpr } else { 1.0 };
        let cap = f64::from(DRAWN_CAP[0].min(DRAWN_CAP[1]));
        let mut k = (self.ratio / dpr).min(cap / w.max(h));
        loop {
            let size = [w, h].map(|v| round(v * k).clamp(1.0, cap) as u32);
            if drawn_fits(size) {
                return (size[0], size[1]);
            }
            // The pixels or the tiles are past their most. Each side's share is brought down by
            // the square root of the tighter of the two ratios, and a step more, so that each
            // pass of the loop takes some off.
            let tiles = [0, 1].map(|a| f64::from(size[a].div_ceil(TILE)));
            let tile_share = f64::from(WORKGROUP_CAP) / (tiles[0] * tiles[1]);
            let area = f64::from(DRAWN_AREA) / (f64::from(size[0]) * f64::from(size[1]));
            k *= libm::sqrt(tile_share.min(area)) * (1.0 - 1.0 / cap);
        }
    }
}

/// The surfaces open, each by the handle the link issued it (spec/gpu/wit
/// surface.wit); a free slot's handle is 0, which the link never issues.
struct Table {
    surfaces: [Surface; SURFACE_CAP],
}

static TABLE: Global<Table> = Global::new();

fn table() -> &'static mut Table {
    TABLE.get(|| Table {
        surfaces: [Surface::default(); SURFACE_CAP],
    })
}

/// Opens the canvas `handle`, which draws nothing until its role is set (`surface_role`).
/// Returns false when every surface is taken.
pub fn surface_open(handle: u32) -> bool {
    let Some(free) = slot_of(0) else {
        return false;
    };
    table().surfaces[free] = Surface {
        handle,
        role: surface_role::NONE,
        dpr: 1.0,
        ratio: 1.0,
        ..Surface::default()
    };
    true
}

/// The slot the surface `handle` takes in the table, which what is kept for each surface is
/// indexed by.
pub fn slot_of(handle: u32) -> Option<usize> {
    table().surfaces.iter().position(|s| s.handle == handle)
}

/// Sets a canvas's size and ratios, and returns whether its handle was live.
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

/// Closes the surface `handle`, and answers the slot it took and what it was.
pub fn surface_close(handle: u32) -> Option<(usize, Surface)> {
    let slot = slot_of(handle).filter(|_| handle != 0)?;
    Some((slot, core::mem::take(&mut table().surfaces[slot])))
}

pub fn surface(handle: u32) -> Option<&'static mut Surface> {
    let slot = slot_of(handle).filter(|_| handle != 0)?;
    Some(&mut table().surfaces[slot])
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
