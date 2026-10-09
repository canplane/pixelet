// pxl-render/gpu/pass/badges.rs

//! Billboards in a pass after the world, with the world's depth attached
//! read-only and bound as a texture. The sheet is premultiplied so its mips
//! filter without a dark fringe; without mips a badge minified slightly against
//! the supersampled sheet crawls with every camera micro-movement.

use crate::frame::draw::{uniform_of, PIPELINE_BADGE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::layers::{mip_levels, Mips};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{Blocks, Draws};
use crate::gpu::sheet::shelf::{DrawSpec, Sheet};
use crate::gpu::sheet::{Align, Halo, SheetPainter, Sketch, SHEET_SCALE};
use crate::gpu::text::{Font, TextOf};

const BADGE_FLOATS: usize = 40;
const SETS: usize = 32;

// Regular and semibold, not canvas's "bold" (700); the system faces carry a real
// semibold, so neither is synthetic.

const REGULAR: u32 = 400;
const SEMIBOLD: u32 = 600;

const BLACK: u32 = 0x000000ff;
const WHITE: u32 = 0xffffffff;

pub const BADGE_FONT: Font = Font { weight: REGULAR, px: 12.0 };
const PILL_INNER: f32 = 20.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 7.0;
const DOT_DIAM: f32 = 6.0;
const DOT_GAP: f32 = 4.0;
const DOT_BORDER: f32 = 1.0;
const PILL_H: f32 = PILL_INNER + PAD_Y;
const PILL_R: f32 = PILL_H / 2.0;
const DOT_CY: f32 = PILL_H + DOT_GAP + DOT_DIAM / 2.0;

fn pill(p: &mut Sketch, logical_w: f32) {
    p.round_rect(0.0, 0.0, logical_w, PILL_H, PILL_R, 0x14141ab8);
}

fn anchor_dot(p: &mut Sketch, logical_w: f32) {
    p.circle(logical_w / 2.0, DOT_CY, DOT_DIAM / 2.0, 0xffffff99);
    p.ring(logical_w / 2.0, DOT_CY, DOT_DIAM / 2.0, DOT_BORDER, 0xffffff40);
}

pub fn badge_width(text_w: f64) -> f64 {
    (text_w + 2.0 * PAD_X as f64).max(PILL_H as f64)
}

// A place as a map app draws one: its symbol white on a disc of its
// category's fill, rimmed white so it holds on any ground, lit from above and
// casting a soft shadow, or before its icon's zoom a small disc of the same; the name under it in its category's
// color, semibold, in a near-opaque white halo. Which places and when are
// Carto's (`pxl-spec bake/place_theme.spec.rs`); how they look is not.

pub const POINT_FONT: Font = Font { weight: SEMIBOLD, px: 12.0 };
/// A symbol's disc across: the symbol (`SYMBOL_PX` at the sheet's scale, 14)
/// with four pixels of fill round it.
const POINT_DISC: f32 = 22.0;
const POINT_DOT: f32 = 8.0;
const POINT_RIM: f32 = 1.5;
const POINT_RIM_FILL: u32 = 0xffffffe6;
/// The fill lighter at the disc's top and darker at its foot by this much.
const POINT_SHADE: f32 = 0.18;
/// A shadow as wide as the disc and a little more, dropped and blurred, and
/// the room it takes round the disc.
const POINT_SHADOW_DY: f32 = 1.5;
const POINT_SHADOW_BLUR: f32 = 3.0;
const POINT_SHADOW_FILL: u32 = 0x00000059;
const POINT_PAD: f32 = 3.0;
/// From the mark's foot to the name's first line.
const POINT_GAP: f32 = 2.0;
const POINT_HALO: f32 = 2.0;
const POINT_HALO_FILL: u32 = 0xfffffff2;
/// Under the halo as under the disc: dropped, and soft past the halo's edge.
const POINT_TEXT_HALO: Halo = Halo {
    color: POINT_HALO_FILL,
    radius: POINT_HALO,
    shadow: 0x00000038,
    shadow_radius: POINT_HALO + 1.5,
    shadow_dy: 1.0,
};
/// A line, the font's size and its halo above and below.
const POINT_LINE: f32 = POINT_FONT.px + 2.0 * POINT_HALO;
/// From one line to the next, as Mapnik sets it (text_line.cpp): the face's
/// own line height plus Carto's @standard-line-spacing-size, −0.15 em. The
/// face is the system's, whose metrics the sheet does not read; 1.2 em is
/// CSS's `line-height: normal` at its high end (CSS 2.1 §10.8.1).
const POINT_PITCH: f32 = (1.2 - 0.15) * POINT_FONT.px;

/// The disc's side and the shadow's room round it.
fn point_mark(icon: bool) -> f32 {
    2.0 * POINT_PAD + if icon { POINT_DISC } else { POINT_DOT }
}

/// A point's sprite, its mark an icon's or a dot's, for a name laid out in
/// `lines` as wide as `text_w`, or none: its width, height and where the
/// place is, down from its top.
pub fn point_size(icon: bool, text: Option<(f64, usize)>) -> [f64; 3] {
    let mark = f64::from(point_mark(icon));
    match text {
        None => [mark, mark, mark / 2.0],
        Some((w, lines)) => {
            // The last line's shadow reaches past its halo.
            let shadow = f64::from(POINT_TEXT_HALO.shadow_radius - POINT_HALO + POINT_TEXT_HALO.shadow_dy);
            let h = mark - f64::from(POINT_PAD) + f64::from(POINT_GAP) + f64::from(POINT_PITCH) * (lines.max(1) - 1) as f64 + f64::from(POINT_LINE) + shadow;
            [(w + 2.0 * f64::from(POINT_TEXT_HALO.shadow_radius)).ceil().max(mark), h.ceil(), mark / 2.0]
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn point(p: &mut Sketch, lines: &[String], icon: bool, symbol: Option<u16>, fill: u32, text: u32, logical_w: f32) {
    let (cx, cy) = (logical_w / 2.0, point_mark(icon) / 2.0);
    let r = cy - POINT_PAD;
    p.soft_circle(cx, cy + POINT_SHADOW_DY, r + 0.5, POINT_SHADOW_BLUR, 0.0, POINT_SHADOW_FILL);
    p.circle(cx, cy, r, POINT_RIM_FILL);
    p.soft_circle(cx, cy, r - POINT_RIM, 1.0 / SHEET_SCALE as f32, POINT_SHADE, fill);
    if let (true, Some(s)) = (icon, symbol) {
        p.icon(usize::from(s), WHITE, cx, cy);
    }
    for (k, line) in lines.iter().enumerate() {
        let ty = 2.0 * cy - POINT_PAD + POINT_GAP + POINT_LINE / 2.0 + POINT_PITCH * k as f32;
        p.haloed_text(POINT_FONT, text, POINT_TEXT_HALO, line, cx, ty, Align::Center);
    }
}

fn labeled_pill(p: &mut Sketch, label: &str, logical_w: f32) {
    pill(p, logical_w);
    p.text(BADGE_FONT, WHITE, label, logical_w / 2.0, PILL_H / 2.0 + 1.0, Align::Center);
    anchor_dot(p, logical_w);
}

pub const BUBBLE_FONT: Font = Font { weight: SEMIBOLD, px: 14.0 };
const PADDING_Y: f32 = 8.0;
const LINE_H: f32 = 17.0;
const BORDER: f32 = 2.0;
const NOTCH: f32 = 3.0;
const TAIL_W: f32 = 14.0;
const TAIL_H: f32 = 10.0;

/// The uncovered corners read as a pixel-art rounded corner; a real arc would
/// anti-alias against nearest sampling.
#[allow(clippy::too_many_arguments)]
fn notched_rect(p: &mut Sketch, x: f32, y: f32, w: f32, h: f32, notch: f32, fill: u32) {
    p.rect(x + notch, y, (w - notch * 2.0).max(0.0), h, fill);
    p.rect(x, y + notch, w, (h - notch * 2.0).max(0.0), fill);
}

fn bubble(p: &mut Sketch, lines: &[String], box_w: f32, box_h: f32) {
    notched_rect(p, 0.0, 0.0, box_w, box_h, NOTCH, BLACK);
    notched_rect(p, BORDER, BORDER, box_w - BORDER * 2.0, box_h - BORDER * 2.0, (NOTCH - BORDER).max(1.0), WHITE);
    let cx = box_w / 2.0;
    p.triangle([cx - TAIL_W / 2.0, box_h - BORDER], [cx + TAIL_W / 2.0, box_h - BORDER], [cx, box_h + TAIL_H - BORDER], BLACK);
    p.triangle(
        [cx - TAIL_W / 2.0 + BORDER * 1.5, box_h - BORDER],
        [cx + TAIL_W / 2.0 - BORDER * 1.5, box_h - BORDER],
        [cx, box_h + TAIL_H - BORDER * 2.5],
        WHITE,
    );
    for (i, line) in lines.iter().enumerate() {
        p.text(BUBBLE_FONT, BLACK, line, box_w / 2.0, PADDING_Y + LINE_H * (i as f32 + 0.5), Align::Center);
    }
}

#[derive(Clone, Copy, Default)]
struct Set {
    uniform: u32,
    group: u32,
    block: u32,
    depth: u32,
}

pub struct BadgePass {
    built: Built,
    painter: SheetPainter,
    sampler: u32,
    sheet: u32,
    sheet_view: u32,
    sheet_group: u32,
    levels: u32,
    generation: u32,
    sets: [Set; SETS],
    dirty: Vec<u32>,
    uniform: [f32; BADGE_FLOATS],
    frame_group: u32,
    pub view: [f32; 16],
    pub proj: [f32; 16],
    pub px_k: f32,
    pub near: f32,
    pub far: f32,
}

impl BadgePass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> BadgePass {
        let sampler = object();
        g.sampler(sampler, filter::NEAREST, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        let built = pipelines.build(g, PIPELINE_BADGE, format::CANVAS);
        let frame_group = object();
        g.bind_group(frame_group, built.layouts[0], &buffer_entry(0, frame_buffer));
        BadgePass {
            built,
            frame_group,
            painter: SheetPainter::new(g, pipelines),
            sampler,
            sheet: 0,
            sheet_view: 0,
            sheet_group: 0,
            levels: 1,
            generation: u32::MAX,
            sets: [Set::default(); SETS],
            dirty: Vec::new(),
            uniform: [0.0; BADGE_FLOATS],
            view: [0.0; 16],
            proj: [0.0; 16],
            px_k: 1.0,
            near: 0.1,
            far: 1000.0,
        }
    }

    /// An entry whose text is not drawn yet is left for a later frame.
    pub fn sync_sheet(&mut self, g: &Gpu, mips: &Mips, sheet: &mut Sheet, text: TextOf) {
        let flush = sheet.flush(self.generation, &mut self.dirty);
        self.generation = flush.generation;
        let live;
        let slots: &[u32] = if flush.whole {
            if flush.size == 0 {
                return;
            }
            for id in [self.sheet_group, self.sheet_view, self.sheet] {
                release(id);
            }
            self.levels = mip_levels(flush.size, flush.size);
            self.sheet = object();
            g.texture(
                self.sheet,
                flush.size,
                flush.size,
                1,
                format::RGBA8_SRGB,
                self.levels,
                texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT,
            );
            self.sheet_view = object();
            g.view(self.sheet_view, self.sheet, dimension::D2, 0, self.levels, 0, 1);
            self.sheet_group = object();
            g.bind_group(
                self.sheet_group,
                self.built.layouts[2],
                &[0, resource::VIEW, self.sheet_view, 0, 0, 1, resource::SAMPLER, self.sampler, 0, 0],
            );
            live = sheet.live();
            &live
        } else {
            &self.dirty
        };
        if slots.is_empty() || self.sheet == 0 {
            return;
        }
        let encoder = object();
        g.encoder(encoder);
        let mut made = Vec::new();
        let mut drawn = false;
        for &slot in slots {
            let Some(spec) = sheet.slot_draw(slot) else { continue };
            let mut p = Sketch::default();
            match spec {
                DrawSpec::Pill { label, logical_w } => labeled_pill(&mut p, label, *logical_w as f32),
                DrawSpec::Point {
                    lines,
                    icon,
                    symbol,
                    fill,
                    text,
                    logical_w,
                } => point(&mut p, lines, *icon, *symbol, *fill, *text, *logical_w as f32),
                DrawSpec::Bubble { lines, box_w, box_h } => bubble(&mut p, lines, *box_w as f32, *box_h as f32),
            }
            if self.painter.paint(g, encoder, self.sheet, sheet.slot_rect(slot), &p, text, &mut made) {
                drawn = true;
            } else {
                sheet.mark_dirty(slot);
            }
        }
        g.submit(encoder);
        release(encoder);
        // After the submit: a texture let go before it would be gone from the commands that read it.
        for id in made {
            release(id);
        }
        if drawn {
            mips.build(g, self.sheet, self.levels, 0, 1);
        }
    }

    /// Each set's uniform and group, made here so drawing makes nothing that
    /// outlives its frame: a set's group holds its block and the depth it reads.
    pub fn prepare(&mut self, g: &Gpu, depth: u32, draws: &Draws, blocks: &Blocks) {
        if self.sheet_group == 0 {
            return;
        }
        let placed = draws.of(PIPELINE_BADGE).filter(|d| d[1] > 0).filter_map(|d| blocks.get(d[0]));
        for (set, (block, _, _)) in self.sets.iter_mut().zip(placed) {
            if set.uniform == 0 {
                set.uniform = object();
                g.buffer(set.uniform, word_bytes(BADGE_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            }
            if set.block != block || set.depth != depth || set.group == 0 {
                release(set.group);
                set.group = object();
                let mut e = [0u32; 15];
                e[..5].copy_from_slice(&buffer_entry(0, set.uniform));
                e[5..10].copy_from_slice(&buffer_entry(1, block));
                e[10..].copy_from_slice(&[2, resource::VIEW, depth, 0, 0]);
                g.bind_group(set.group, self.built.layouts[1], &e);
                set.block = block;
                set.depth = depth;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, g: &Gpu, encoder: u32, color: u32, depth: u32, draws: &Draws, uniforms: &[f32], blocks: &Blocks, stamp: &mut Stamp) {
        if self.sheet_group == 0 {
            return;
        }
        let mut pass = 0;
        let placed = draws.of(PIPELINE_BADGE).filter(|d| d[1] > 0).filter(|d| blocks.get(d[0]).is_some());
        // A uniform buffer a set: every write to one buffer lands before the pass, so
        // a shared one would place every set where the last is.
        for (set, d) in self.sets.iter().zip(placed) {
            let s = &mut self.uniform;
            s[..16].copy_from_slice(&self.view);
            s[16..32].copy_from_slice(&self.proj);
            s[32..35].copy_from_slice(&uniform_of(uniforms, d)[..3]);
            s[35..40].copy_from_slice(&[self.px_k, self.near, self.far, 0.0, 0.0]);
            g.write_buffer(set.uniform, 0, bytes(s));
            if pass == 0 {
                pass = object();
                g.pass(pass, encoder, &pass_desc(color, load::LOAD, depth, load::READ_ONLY, 0.0, stamp.pass()));
                g.set_pipeline(pass, self.built.pipeline);
                g.set_bind_group(pass, 0, self.frame_group, &[]);
                g.set_bind_group(pass, 2, self.sheet_group, &[]);
            }
            g.set_bind_group(pass, 1, set.group, &[]);
            g.draw(pass, 4, d[1], 0, 0);
        }
        if pass != 0 {
            g.end(pass);
            release(pass);
        }
    }
}
