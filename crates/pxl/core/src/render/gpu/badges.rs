// pxl_core/render/gpu/badges.rs

//! The billboards on the device — the POI badges and the chat bubble: one
//! instanced quad over the scheduler's billboard sets (module/world/billboard/field.rs), one
//! draw a set, in a pass of their own after the world with the world's depth
//! attached read-only and bound as a texture at once (shader/badge.wgsl asks
//! both of its depth questions of this frame's depth).
//!
//! The sheet is the kernel's billboard atlas (module/world/billboard/atlas.rs),
//! painted into a texture an entry at a time and whole when the atlas replaced
//! the sheet, with its mip chain rebuilt after. An entry is painted through the
//! host's 2D scratch (paint.rs) — the fonts are the host's — as the atlas's draw
//! spec says, and copied into its rectangle. Magnification is nearest, for the
//! pixel read up close; minification is mipmapped linear, because a badge held
//! at a constant screen size is still slightly minified against the sheet's
//! supersampled resolution, and without a mip chain every camera
//! micro-movement samples a different single texel and the text visibly crawls.
//!
//! Uploads are rectangles, not the sheet: re-uploading the whole sheet whenever
//! a badge was drawn cost 64 MB at 4096 px once for every frame a badge was
//! drawn in, and a phone paid it as a hundred-millisecond task each time.

use core::f32::consts::TAU;

use super::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, GpuApi, NONE};
use super::api_spec::{address, compare, dimension, filter, format, load, resource};
use super::layers::{mip_levels, Mips};
use super::paint::Paint;
use super::pipeline::{Built, Pipelines};
use super::renderer::Blocks;
use crate::module::world::billboard::atlas::{DrawSpec, PAD};
use crate::module::world::billboard::sheet::billboard_sheet;
use crate::render::draw::{DRAW_WORDS, PIPELINE_BADGE, UNIFORM_FLOATS};

/// Floats the badge uniform holds (shader/badge.wgsl's Badge).
const BADGE_FLOATS: usize = 44;
/// Sets drawn in one frame at most.
const SETS: usize = 32;
/// Supersampling: the renderer's device pixel ratio, clamped to 2.
const RENDER_SCALE: f32 = 2.0;

macro_rules! font_family {
    () => {
        "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"SF Pro Display\", \"Pretendard Variable\", system-ui, sans-serif"
    };
}

// ---- a place and a cluster (module/world/poi/poi_badge.rs) ----

const BADGE_FONT: &str = concat!("400 12px ", font_family!());
const ICON_FONT: &str = "14px sans-serif";
const ICON_SIZE: f32 = 20.0;
const GAP: f32 = 8.0;
const PAD_Y: f32 = 7.0;
const DOT_DIAM: f32 = 6.0;
const DOT_GAP: f32 = 4.0;
const DOT_BORDER: f32 = 1.0;
const PILL_H: f32 = ICON_SIZE + PAD_Y;
const PILL_R: f32 = PILL_H / 2.0;
const TEXT_X: f32 = PILL_R + ICON_SIZE / 2.0 + GAP;
const DOT_CY: f32 = PILL_H + DOT_GAP + DOT_DIAM / 2.0;

const LEFT: u32 = 0;
const CENTER: u32 = 1;
const MIDDLE: u32 = 0;

/// The pill every badge stands on: fully-rounded ends.
fn pill(p: &mut Paint, logical_w: f32) {
    p.fill_style("rgba(20,20,26,0.72)");
    p.begin_path();
    p.round_rect(0.0, 0.0, logical_w, PILL_H, PILL_R);
    p.fill();
}

/// The anchor dot, centred under the pill, marking the ground point it floats above.
fn anchor_dot(p: &mut Paint, logical_w: f32) {
    p.fill_style("rgba(255,255,255,0.6)");
    p.begin_path();
    p.arc(logical_w / 2.0, DOT_CY, DOT_DIAM / 2.0, 0.0, TAU);
    p.fill();
    p.stroke_style("rgba(255,255,255,0.25)");
    p.line_width(DOT_BORDER);
    p.stroke();
}

fn place<'a>(p: &mut Paint<'a>, icon: &'a str, color: &'a str, name: &'a str, logical_w: f32) {
    pill(p, logical_w);
    // The icon circle is concentric with the pill's rounded left cap.
    let (cx, cy) = (PILL_R, PILL_H / 2.0);
    p.fill_style(color);
    p.begin_path();
    p.arc(cx, cy, ICON_SIZE / 2.0, 0.0, TAU);
    p.fill();
    p.font(ICON_FONT);
    p.text_align(CENTER);
    p.text_baseline(MIDDLE);
    p.fill_text(icon, cx, cy + 1.0);
    p.font(BADGE_FONT);
    p.text_align(LEFT);
    p.fill_style("#fff");
    p.fill_text(name, TEXT_X, PILL_H / 2.0 + 1.0);
    anchor_dot(p, logical_w);
}

fn cluster<'a>(p: &mut Paint<'a>, label: &'a str, logical_w: f32) {
    pill(p, logical_w);
    p.font(BADGE_FONT);
    p.text_align(CENTER);
    p.text_baseline(MIDDLE);
    p.fill_style("#fff");
    p.fill_text(label, logical_w / 2.0, PILL_H / 2.0 + 1.0);
    anchor_dot(p, logical_w);
}

// ---- a speech bubble (module/world/character/chat_bubble.rs) ----

const BUBBLE_FONT: &str = concat!("600 14px ", font_family!());
const PADDING_Y: f32 = 8.0;
const LINE_H: f32 = 17.0;
const BORDER: f32 = 2.0;
const NOTCH: f32 = 3.0;
const TAIL_W: f32 = 14.0;
const TAIL_H: f32 = 10.0;

/// A plus-shaped union of two overlapping rectangles: the corners left
/// uncovered read as a chunky, pixel-art rounded corner without a real arc,
/// which would anti-alias against nearest sampling.
#[allow(clippy::too_many_arguments)]
fn notched_rect<'a>(p: &mut Paint<'a>, x: f32, y: f32, w: f32, h: f32, notch: f32, fill: &'a str) {
    p.fill_style(fill);
    p.fill_rect(x + notch, y, (w - notch * 2.0).max(0.0), h);
    p.fill_rect(x, y + notch, w, (h - notch * 2.0).max(0.0));
}

fn bubble<'a>(p: &mut Paint<'a>, lines: &'a [String], box_w: f32, box_h: f32) {
    notched_rect(p, 0.0, 0.0, box_w, box_h, NOTCH, "#000");
    notched_rect(p, BORDER, BORDER, box_w - BORDER * 2.0, box_h - BORDER * 2.0, (NOTCH - BORDER).max(1.0), "#fff");
    let cx = box_w / 2.0;
    p.fill_style("#000");
    p.begin_path();
    p.move_to(cx - TAIL_W / 2.0, box_h - BORDER);
    p.line_to(cx + TAIL_W / 2.0, box_h - BORDER);
    p.line_to(cx, box_h + TAIL_H - BORDER);
    p.close_path();
    p.fill();
    p.fill_style("#fff");
    p.begin_path();
    p.move_to(cx - TAIL_W / 2.0 + BORDER * 1.5, box_h - BORDER);
    p.line_to(cx + TAIL_W / 2.0 - BORDER * 1.5, box_h - BORDER);
    p.line_to(cx, box_h + TAIL_H - BORDER * 2.5);
    p.close_path();
    p.fill();
    p.font(BUBBLE_FONT);
    p.fill_style("#000");
    p.text_align(CENTER);
    p.text_baseline(MIDDLE);
    for (i, line) in lines.iter().enumerate() {
        p.fill_text(line, box_w / 2.0, PADDING_Y + LINE_H * (i as f32 + 0.5));
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
    sampler: u32,
    sheet: u32,
    sheet_view: u32,
    sheet_group: u32,
    levels: u32,
    generation: u32,
    sets: [Set; SETS],
    uniform: [f32; BADGE_FLOATS],
    /// The frame's inputs: the view in the anchor root's frame and the
    /// projection into WebGPU's clip space, the projection's pixels per unit at
    /// one unit of depth, the fade's range, the camera's near and far, and the
    /// tier ladder's cell per distance.
    pub view: [f32; 16],
    pub proj: [f32; 16],
    pub px_k: f32,
    pub fade_start: f32,
    pub fade_end: f32,
    pub near: f32,
    pub far: f32,
    pub cell: f32,
}

impl BadgePass {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32) -> BadgePass {
        let sampler = object();
        (g.sampler)(sampler, filter::NEAREST, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        BadgePass {
            built: pipelines.build(g, PIPELINE_BADGE, format::CANVAS, samples),
            sampler,
            sheet: 0,
            sheet_view: 0,
            sheet_group: 0,
            levels: 1,
            generation: u32::MAX,
            sets: [Set::default(); SETS],
            uniform: [0.0; BADGE_FLOATS],
            view: [0.0; 16],
            proj: [0.0; 16],
            px_k: 1.0,
            fade_start: 0.0,
            fade_end: 1.0,
            near: 0.1,
            far: 1000.0,
            cell: 0.01,
        }
    }

    /// The frame's depth is multisampled as the frame is, so a set's group is
    /// laid out anew.
    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.built = pipelines.build(g, PIPELINE_BADGE, format::CANVAS, samples);
        for s in &mut self.sets {
            release(s.group);
            s.group = 0;
            s.depth = 0;
        }
    }

    /// Paints what the atlas changed since the texture last looked, and
    /// rebuilds its mip chain when anything moved.
    pub fn sync_sheet(&mut self, g: &GpuApi, mips: &Mips) {
        let atlas = billboard_sheet();
        let flush = atlas.flush(self.generation);
        self.generation = flush.generation;
        let slots = if flush.whole {
            if flush.size == 0 {
                return;
            }
            for id in [self.sheet_group, self.sheet_view, self.sheet] {
                release(id);
            }
            self.levels = mip_levels(flush.size, flush.size);
            self.sheet = object();
            (g.texture)(
                self.sheet,
                flush.size,
                flush.size,
                1,
                format::RGBA8_SRGB,
                self.levels,
                1,
                texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT,
            );
            self.sheet_view = object();
            (g.view)(self.sheet_view, self.sheet, dimension::D2, 0, self.levels, 0, 1);
            self.sheet_group = object();
            (g.bind_group)(
                self.sheet_group,
                self.built.layouts[1],
                &[0, resource::VIEW, self.sheet_view, 0, 0, 1, resource::SAMPLER, self.sampler, 0, 0],
            );
            atlas.live()
        } else {
            flush.dirty
        };
        if slots.is_empty() || self.sheet == 0 {
            return;
        }
        for &slot in &slots {
            let [x, y, w, h] = atlas.slot_rect(slot);
            let Some(spec) = atlas.slot_draw(slot) else { continue };
            let mut p = Paint::new();
            p.size(w, h);
            p.save();
            p.translate(PAD as f32, PAD as f32);
            p.smoothing(false);
            p.scale(RENDER_SCALE, RENDER_SCALE);
            match spec {
                DrawSpec::PoiBadge { icon, color, name, logical_w } => place(&mut p, icon, color, name, *logical_w as f32),
                DrawSpec::Cluster { label, logical_w } => cluster(&mut p, label, *logical_w as f32),
                DrawSpec::Bubble { lines, box_w, box_h } => bubble(&mut p, lines, *box_w as f32, *box_h as f32),
            }
            p.restore();
            p.run(g);
            (g.paint_copy)(self.sheet, x, y, 0, w, h);
        }
        mips.build(g, self.sheet, self.levels, 0, 1);
    }

    /// Every billboard set's draw, in the order listed, in one pass of their
    /// own over `color` (resolved into `resolve` where the colour is
    /// multisampled) reading `depth`; how many instances it drew.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, g: &GpuApi, encoder: u32, color: u32, resolve: u32, depth: u32, viewport: [u32; 2], draws: &[u32], uniforms: &[f32], blocks: &Blocks) -> u32 {
        if self.sheet_group == 0 {
            return 0;
        }
        let mut pass = 0;
        let mut n = 0;
        let mut drawn = 0;
        for d in draws.chunks_exact(DRAW_WORDS).filter(|d| d[4] == PIPELINE_BADGE && d[1] > 0) {
            let Some((block, _, _)) = blocks.get(d[0]) else { continue };
            if n == SETS {
                break;
            }
            // A uniform buffer a set: writes to one buffer all land before the
            // pass runs, so a shared one would place every set where the last is.
            let set = &mut self.sets[n];
            n += 1;
            if set.uniform == 0 {
                set.uniform = object();
                (g.buffer)(set.uniform, (BADGE_FLOATS * 4) as u32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            }
            let at = d[3] as usize * UNIFORM_FLOATS;
            let s = &mut self.uniform;
            s[..16].copy_from_slice(&self.view);
            s[16..32].copy_from_slice(&self.proj);
            s[32..35].copy_from_slice(&uniforms[at..at + 3]);
            s[35..44].copy_from_slice(&[self.px_k, self.fade_start, self.fade_end, self.near, self.far, self.cell, 1.0, viewport[0] as f32, viewport[1] as f32]);
            (g.write_buffer)(set.uniform, 0, bytes(s));
            if set.block != block || set.depth != depth || set.group == 0 {
                release(set.group);
                set.group = object();
                let mut e = [0u32; 15];
                e[..5].copy_from_slice(&buffer_entry(0, set.uniform));
                e[5..10].copy_from_slice(&buffer_entry(1, block));
                e[10..].copy_from_slice(&[2, resource::VIEW, depth, 0, 0]);
                (g.bind_group)(set.group, self.built.layouts[0], &e);
                set.block = block;
                set.depth = depth;
            }
            if pass == 0 {
                pass = object();
                (g.pass)(pass, encoder, &pass_desc(color, resolve, load::LOAD, depth, load::READ_ONLY, 0.0, 0, NONE, NONE));
                (g.set_pipeline)(pass, self.built.pipeline);
                (g.set_bind_group)(pass, 1, self.sheet_group, &[]);
            }
            (g.set_bind_group)(pass, 0, set.group, &[]);
            (g.draw)(pass, 4, d[1], 0, 0);
            drawn += d[1];
        }
        if pass != 0 {
            (g.end)(pass);
            release(pass);
        }
        drawn
    }
}
