// pxl-render/gpu/sheet.rs

//! A billboard sheet entry drawn on the device into a texture of its size and
//! copied into its rectangle. Sketches are in logical pixels, drawn at the
//! sheet's scale.

pub mod shelf;

use crate::frame::shader::record::PIPELINE_SHEET;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{buffer_entry, buffer_usage, bytes, pass_desc, room, sampler_entry, texture_usage, view_entry, write_buffer_bytes, write_texture_bytes, Format, Gpu, Load};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::sheet::shelf::PAD;
use crate::gpu::text::{Font, Raster, TextOf};
use pxl_spec::asset::symbol::{SYMBOL_MASKS, SYMBOL_PX};
use xpute_kit::math::scalar::round;

const SHAPE_FLOATS: usize = 20;

const FILL: u32 = 0;
const STROKE: u32 = 1;
const TRIANGLE: u32 = 2;
const TEXT: u32 = 3;
const HALO: u32 = 4;

struct Line {
    font: Font,
    color: u32,
    text: String,
    /// Its middle across, and its baseline's height.
    at: [f32; 2],
    halo: Option<Halo>,
}

/// A color round a line's glyphs and its radius, and under it a shadow, its
/// color, radius and drop; in logical pixels.
#[derive(Clone, Copy)]
pub struct Halo {
    pub color: u32,
    pub radius: f32,
    pub shadow: u32,
    pub shadow_radius: f32,
    pub shadow_dy: f32,
}

/// Device pixels a sheet draws per logical pixel: past two, texels cost more
/// than the eye resolves at arm's length. The world is drawn no finer
/// (pxl-kernel param/view.rs `RESOLUTION_MAX`), so a label is never shown
/// smaller than drawn.
pub const SHEET_SCALE: f64 = 2.0;

#[derive(Default)]
pub struct Sketch {
    shapes: Vec<[f32; SHAPE_FLOATS]>,
    lines: Vec<Line>,
    icons: Vec<Icon>,
}

/// One of Carto's symbols (pxl-spec asset/symbol.spec.rs), centered at
/// `at`, its mask in `color`.
struct Icon {
    symbol: usize,
    color: u32,
    at: [f32; 2],
}

/// A symbol's mask in `color`, as the platform hands text: RGBA, not
/// premultiplied, top row first.
fn tinted(symbol: usize, color: u32) -> Vec<u8> {
    let side = (SYMBOL_PX * SYMBOL_PX) as usize;
    let mask = &SYMBOL_MASKS[symbol * side..(symbol + 1) * side];
    let [r, g, b, a] = color.to_be_bytes();
    mask.iter().flat_map(|&m| [r, g, b, ((u32::from(m) * u32::from(a) + 127) / 255) as u8]).collect()
}

fn px(v: f32) -> f32 {
    PAD as f32 + v * SHEET_SCALE as f32
}

/// The texture is sRGB, and what a shader writes into it is linear.
fn linear(rgba: u32) -> [f32; 4] {
    let channel = |shift: u32| {
        let c = ((rgba >> shift) & 0xff) as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            libm::powf((c + 0.055) / 1.055, 2.4)
        }
    };
    [channel(24), channel(16), channel(8), (rgba & 0xff) as f32 / 255.0]
}

fn shape(kind: u32, rect: [f32; 4], color: u32, p: [f32; 4], q: [f32; 2]) -> [f32; SHAPE_FLOATS] {
    let mut s = [0.0; SHAPE_FLOATS];
    s[..4].copy_from_slice(&rect);
    s[4..8].copy_from_slice(&linear(color));
    s[8..12].copy_from_slice(&p);
    s[12..14].copy_from_slice(&q);
    s[16] = f32::from_bits(kind);
    s
}

impl Sketch {
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        let rect = [px(x), px(y), px(x + w), px(y + h)];
        self.shapes.push(shape(FILL, rect, color, [r * SHEET_SCALE as f32, 0.0, 0.0, 0.0], [0.0; 2]));
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        self.round_rect(x, y, w, h, 0.0, color);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: u32) {
        self.round_rect(cx - r, cy - r, 2.0 * r, 2.0 * r, r, color);
    }

    /// A disc whose edge fades over `edge` and whose fill is `shade` lighter
    /// at its top and as much darker at its foot.
    #[allow(clippy::too_many_arguments)]
    pub fn soft_circle(&mut self, cx: f32, cy: f32, r: f32, edge: f32, shade: f32, color: u32) {
        let rect = [px(cx - r), px(cy - r), px(cx + r), px(cy + r)];
        let scale = SHEET_SCALE as f32;
        self.shapes.push(shape(FILL, rect, color, [r * scale, edge * scale, 0.0, 0.0], [shade, 0.0]));
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, color: u32) {
        let rect = [px(cx - r), px(cy - r), px(cx + r), px(cy + r)];
        let scale = SHEET_SCALE as f32;
        self.shapes.push(shape(STROKE, rect, color, [r * scale, width * scale, 0.0, 0.0], [0.0; 2]));
    }

    pub fn triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: u32) {
        let [a, b, c] = [a, b, c].map(|[x, y]| [px(x), px(y)]);
        let rect = [a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1]), a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1])];
        self.shapes.push(shape(TRIANGLE, rect, color, [a[0], a[1], b[0], b[1]], c));
    }

    /// Carto's `symbol`, by its place in `SYMBOLS`, centered at (cx, cy).
    pub fn icon(&mut self, symbol: usize, color: u32, cx: f32, cy: f32) {
        self.icons.push(Icon { symbol, color, at: [cx, cy] });
    }

    /// A line centered across on `x`.
    pub fn text(&mut self, font: Font, color: u32, text: &str, x: f32, y: f32) {
        self.lines.push(Line {
            font,
            color,
            text: text.to_owned(),
            at: [x, y],
            halo: None,
        });
    }

    /// Text over its own glyphs grown into `halo`, under every line's text.
    pub fn haloed_text(&mut self, font: Font, color: u32, halo: Halo, text: &str, x: f32, y: f32) {
        self.text(font, color, text, x, y);
        if let Some(line) = self.lines.last_mut() {
            line.halo = Some(halo);
        }
    }
}

pub struct SheetPainter {
    built: Built,
    sampler: u32,
}

impl SheetPainter {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> SheetPainter {
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::nearest, AddressMode::clamp_to_edge, None, 1);
        SheetPainter {
            built: pipelines.build(g, PIPELINE_SHEET, Format::rgba8unorm_srgb),
            sampler,
        }
    }

    /// The rasters of a sketch's lines, as the host has drawn them; none while
    /// one is not drawn yet, whose landing is the turn that paints it.
    pub fn rasters(sketch: &Sketch, text: TextOf) -> Option<Vec<Raster<'static>>> {
        sketch.lines.iter().map(|line| text(line.font, line.color, &line.text)).collect()
    }

    /// `sketch` painted with its lines' `rasters` (`rasters`); false, and
    /// nothing recorded, where its pixels do not fit this turn's stream
    /// (`room`).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(&self, g: &Gpu, encoder: u32, sheet: u32, rect: [u32; 4], sketch: &Sketch, rasters: Vec<Raster<'static>>, made: &mut Vec<u32>) -> bool {
        let icons: Vec<Vec<u8>> = sketch.icons.iter().map(|i| tinted(i.symbol, i.color)).collect();
        let drawn = rasters.len() + icons.len();
        let [x, y, w, h] = rect;

        let icon_side = if icons.is_empty() { 1 } else { SYMBOL_PX };
        let layer_w = rasters.iter().map(|r| r.width).max().unwrap_or(1).max(icon_side);
        let layer_h = rasters.iter().map(|r| r.height).max().unwrap_or(1).max(icon_side);
        let layers = (drawn as u32).max(1);
        let mut shapes = sketch.shapes.clone();
        let placed: Vec<([f32; 4], [f32; 4])> = sketch
            .lines
            .iter()
            .zip(&rasters)
            .enumerate()
            .map(|(layer, (line, r))| {
                let x0 = round(px(line.at[0]) - r.advance / 2.0 - r.origin[0]);
                let y0 = round(px(line.at[1]) - r.origin[1]);
                let place = [x0, y0, x0 + r.width as f32, y0 + r.height as f32];
                (place, [layer as f32, r.width as f32 / layer_w as f32, r.height as f32 / layer_h as f32, 0.0])
            })
            .collect();
        // Every shadow, then every halo, so one line's never covers another's.
        let scale = SHEET_SCALE as f32;
        for (line, &(place, cover)) in sketch.lines.iter().zip(&placed) {
            if let Some(h) = line.halo {
                let dy = round(h.shadow_dy * scale);
                let dropped = [place[0], place[1] + dy, place[2], place[3] + dy];
                // Soft over all of its radius past the halo's.
                let soft = (h.shadow_radius - h.radius) * scale;
                shapes.push(shape(HALO, dropped, h.shadow, [cover[0], cover[1], cover[2], h.shadow_radius * scale], [soft, 0.0]));
            }
        }
        for (line, &(place, cover)) in sketch.lines.iter().zip(&placed) {
            if let Some(h) = line.halo {
                shapes.push(shape(HALO, place, h.color, [cover[0], cover[1], cover[2], h.radius * scale], [0.0; 2]));
            }
        }
        for &(place, cover) in &placed {
            shapes.push(shape(TEXT, place, 0, cover, [0.0; 2]));
        }
        for (k, icon) in sketch.icons.iter().enumerate() {
            let layer = (rasters.len() + k) as u32;
            let half = SYMBOL_PX as f32 / 2.0;
            let (x0, y0) = (round(px(icon.at[0]) - half), round(px(icon.at[1]) - half));
            let place = [x0, y0, x0 + SYMBOL_PX as f32, y0 + SYMBOL_PX as f32];
            let cover = [layer as f32, SYMBOL_PX as f32 / layer_w as f32, SYMBOL_PX as f32 / layer_h as f32, 0.0];
            shapes.push(shape(TEXT, place, 0, cover, [0.0; 2]));
        }
        // What goes into the stream whole, asked of it at once: the text's and
        // the icons' pixels, the size and the shapes, as their writers measure them.
        let size_bytes = [w as f32, h as f32, 0.0, 0.0];
        let instance_bytes = bytes(shapes.as_flattened());
        let pixels = rasters.iter().map(|r| r.pixels).chain(icons.iter().map(Vec::as_slice));
        if !room(pixels.map(write_texture_bytes).sum::<usize>() + write_buffer_bytes(bytes(&size_bytes)) + write_buffer_bytes(instance_bytes)) {
            return false;
        }
        let lines = g.texture(layer_w, layer_h, layers, Format::rgba8unorm_srgb, 1, texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST);
        for (layer, r) in rasters.iter().enumerate() {
            g.write_texture(lines, layer as u32, r.width, r.height, r.pixels);
        }
        for (k, pixels) in icons.iter().enumerate() {
            g.write_texture(lines, (rasters.len() + k) as u32, SYMBOL_PX, SYMBOL_PX, pixels);
        }
        let lines_view = g.view(lines, true, 0, 1, 0, layers);

        let size = g.buffer(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        g.write_buffer(size, 0, bytes(&size_bytes));
        let instances = g.buffer(instance_bytes.len().max(16) as u32, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(instances, 0, instance_bytes);

        let entry_group = g.bind_group(self.built.layouts[0], &[buffer_entry(0, size), buffer_entry(1, instances)]);
        let lines_group = g.bind_group(self.built.layouts[1], &[view_entry(0, lines_view), sampler_entry(1, self.sampler)]);

        let target = g.texture(w, h, 1, Format::rgba8unorm_srgb, 1, texture_usage::RENDER_ATTACHMENT | texture_usage::COPY_SRC);
        let target_view = g.view(target, false, 0, 1, 0, 1);
        let pass = g.pass(encoder, &pass_desc(target_view, Load::Clear, 0, Load::Clear, 0.0, None));
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, entry_group, &[]);
        g.set_bind_group(pass, 1, lines_group, &[]);
        g.draw(pass, 4, shapes.len() as u32, 0, 0);
        g.end(pass);
        g.copy_texture(encoder, target, 0, 0, sheet, x, y, 0, w, h);
        made.extend([pass, target_view, target, lines_group, entry_group, instances, size, lines_view, lines]);
        true
    }
}
