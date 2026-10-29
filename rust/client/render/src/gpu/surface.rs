// pxl-render/gpu/surface.rs

//! The canvases the kernel draws into, each at its display's device pixels,
//! the world inside it at its own (`render_px`). The page scales whatever
//! buffer exists into the layout box, so a stale one still shows whole.

use xpute_guest::abi::handle::{handle_slot, Table as Handles};
use xpute_guest::global::Global;
use xpute_kit::math::scalar::round;

use crate::gpu::api::Format;

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

/// A part of the world's drawn pixels, from `at`, `size` across and down:
/// what a frame draws again where nothing moved but what the kernel's region
/// holds (gpu/renderer.rs `FrameInput::region`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub at: [u32; 2],
    pub sz: [u32; 2],
}

/// The drawn pixels a region is grown by each way: the resolve's pixels
/// beside it and a face's growth (pxl-spec-core param.rs `raster::GROWN`),
/// with room.
const REGION_MARGIN: u32 = 4;

/// The share of the world's pixels past which a region is drawn whole.
const REGION_MOST: f64 = 0.5;

impl Region {
    /// The drawn pixels under clip `[x0, y0, x1, y1]`, Y up, of a world
    /// `width` by `height`, grown by `REGION_MARGIN`; none where they hold
    /// more than `REGION_MOST` of it, or nothing.
    pub fn of([x0, y0, x1, y1]: [f32; 4], width: u32, height: u32) -> Option<Region> {
        let (w, h) = (width as f32, height as f32);
        let px = |x: f32| (x * 0.5 + 0.5) * w;
        let py = |y: f32| (0.5 - y * 0.5) * h;
        let m = REGION_MARGIN as f32;
        let lo = [(px(x0) - m).max(0.0), (py(y1) - m).max(0.0)];
        let hi = [(px(x1) + m).min(w), (py(y0) + m).min(h)];
        if !(hi[0] > lo[0] && hi[1] > lo[1]) {
            return None;
        }
        // Whole workgroups of the pixels' cast (pxl-shader cast.rs
        // `cast_cs`, 8 by 8), so its last ones cast no pixel past it.
        let whole = |lo: f32, hi: f32, n: u32| {
            let sz = (hi.ceil() as u32 - lo as u32).next_multiple_of(8).min(n);
            [(lo as u32).min(n - sz), sz]
        };
        let ([ax, sx], [ay, sy]) = (whole(lo[0], hi[0], width), whole(lo[1], hi[1], height));
        let (at, sz) = ([ax, ay], [sx, sy]);
        let share = f64::from(sz[0]) * f64::from(sz[1]) / (f64::from(w) * f64::from(h));
        (share <= REGION_MOST).then_some(Region { at, sz })
    }

    /// As a pass's scissor takes it.
    pub fn rect(&self) -> [u32; 4] {
        [self.at[0], self.at[1], self.sz[0], self.sz[1]]
    }
}

#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub handle: u32,
    pub role: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub dpr: f64,
    /// The device pixels per CSS pixel the kernel has it drawn at (pxl-kernel
    /// domain/view/frame/resolution.rs): the display's, or under load a share of
    /// them, the finish bringing them up to the canvas's (`render_px`).
    pub ratio: f64,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    /// The world's size and the canvas's last asked, which hold
    /// `SETTLE_TURNS` before the targets are made at them.
    pub wanted: ((u32, u32), (u32, u32)),
    pub waited: u32,
    /// The frame's depth, the world's and over it what moves on its own
    /// (gpu/renderer.rs, stage 2).
    pub depth: u32,
    pub depth_view: u32,
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
    /// layer (pxl-spec-core `ray`).
    pub rays: u32,
    pub rays_view: u32,
    /// The world's color gathered over frames, two of `WORLD_COLOR` made
    /// with the depth, the last frame's read as this one's is written, in
    /// turn (gpu/resolve.rs): the world's alone.
    pub history: [u32; 2],
    pub history_view: [u32; 2],
}

impl Surface {
    /// What it holds on the GPU, every one made again with its size.
    pub fn targets(&self) -> [u32; 12] {
        [
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
        ]
    }

    /// The canvas's device pixels: the display's, whatever the world is drawn
    /// at.
    pub fn screen_px(&self) -> (u32, u32) {
        (self.px_w.max(1), self.px_h.max(1))
    }

    /// The world's pixels: the canvas's at the kernel's share of them
    /// (`ratio` over the display's), which the finish brings up to the
    /// canvas.
    pub fn render_px(&self) -> (u32, u32) {
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
