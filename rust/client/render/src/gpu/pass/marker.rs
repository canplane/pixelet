// pxl-render/gpu/pass/marker.rs

//! Markers in two passes after the world, as billboards: their regions,
//! each as its marker's number, into a target of their own, then the markers whose
//! number is the one at their anchor, with the world's depth attached
//! read-only (marker.wgsl). The sheet is premultiplied so its mips
//! filter without a dark fringe; without mips a marker minified slightly against
//! the supersampled sheet crawls with every camera micro-movement.

use crate::frame::draw::{uniform_of, PIPELINE_MARKER};
use crate::frame::shader::record::PIPELINE_MARKER_REGION;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::word::{marker, MARKER_FLOATS};
use crate::gpu::api::{
    buffer_entry, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes,
    Format, Gpu, Load, Stamp,
};
use crate::gpu::mip::{mip_levels, Mips};
use crate::gpu::pass::model::DrawUniforms;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::sheet::shelf::{Paint, Sheet, SlotSet};
use crate::gpu::sheet::{SheetPainter, Sketch, SHEET_SCALE};
use crate::gpu::text::TextOf;
use crate::gpu::upload::{Draws, Uploads};
use crate::look::marker::{
    bubble_box_h, point_mark, BLACK, BORDER, BUBBLE_FONT, DOT_BORDER, DOT_CY, DOT_DIAM, DOT_FILL,
    DOT_RING, LINE_H, NOTCH, PADDING_Y, PILL_FONT, PILL_H, PILL_R, POINT_FONT, POINT_GAP,
    POINT_LINE, POINT_PAD, POINT_PITCH, POINT_RIM, POINT_RIM_FILL, POINT_SHADE, POINT_SHADOW_BLUR,
    POINT_SHADOW_DY, POINT_SHADOW_FILL, POINT_TEXT_HALO, TAIL_H, TAIL_W, WHITE,
};

const SETS: usize = 32;

fn anchor_dot(p: &mut Sketch, logical_w: f32) {
    p.circle(logical_w / 2.0, DOT_CY, DOT_DIAM / 2.0, DOT_FILL);
    p.ring(
        logical_w / 2.0,
        DOT_CY,
        DOT_DIAM / 2.0,
        DOT_BORDER,
        DOT_RING,
    );
}

#[allow(clippy::too_many_arguments)]
fn point(
    p: &mut Sketch,
    lines: &[String],
    icon: bool,
    symbol: Option<u16>,
    fill: u32,
    ink: u32,
    logical_w: f32,
) {
    let (cx, cy) = (logical_w / 2.0, point_mark(icon) / 2.0);
    let r = cy - POINT_PAD;
    p.soft_circle(
        cx,
        cy + POINT_SHADOW_DY,
        r + 0.5,
        POINT_SHADOW_BLUR,
        0.0,
        POINT_SHADOW_FILL,
    );
    p.circle(cx, cy, r, POINT_RIM_FILL);
    p.soft_circle(
        cx,
        cy,
        r - POINT_RIM,
        1.0 / SHEET_SCALE as f32,
        POINT_SHADE,
        fill,
    );
    if let (true, Some(s)) = (icon, symbol) {
        p.icon(usize::from(s), WHITE, cx, cy);
    }
    for (k, line) in lines.iter().enumerate() {
        let ty = 2.0 * cy - POINT_PAD + POINT_GAP + POINT_LINE / 2.0 + POINT_PITCH * k as f32;
        p.haloed_text(POINT_FONT, ink, POINT_TEXT_HALO, line, cx, ty);
    }
}

fn pill(p: &mut Sketch, words: &str, fill: u32, ink: u32, logical_w: f32) {
    p.round_rect(0.0, 0.0, logical_w, PILL_H, PILL_R, fill);
    p.text(PILL_FONT, ink, words, logical_w / 2.0, PILL_H / 2.0 + 1.0);
    anchor_dot(p, logical_w);
}

/// The uncovered corners read as a pixel-art rounded corner; a real arc would
/// anti-alias against nearest sampling.
#[allow(clippy::too_many_arguments)]
fn notched_rect(p: &mut Sketch, x: f32, y: f32, w: f32, h: f32, notch: f32, fill: u32) {
    p.rect(x + notch, y, (w - notch * 2.0).max(0.0), h, fill);
    p.rect(x, y + notch, w, (h - notch * 2.0).max(0.0), fill);
}

fn bubble(p: &mut Sketch, lines: &[String], box_w: f32) {
    let box_h = bubble_box_h(lines.len());
    notched_rect(p, 0.0, 0.0, box_w, box_h, NOTCH, BLACK);
    notched_rect(
        p,
        BORDER,
        BORDER,
        box_w - BORDER * 2.0,
        box_h - BORDER * 2.0,
        (NOTCH - BORDER).max(1.0),
        WHITE,
    );
    let cx = box_w / 2.0;
    p.triangle(
        [cx - TAIL_W / 2.0, box_h - BORDER],
        [cx + TAIL_W / 2.0, box_h - BORDER],
        [cx, box_h + TAIL_H - BORDER],
        BLACK,
    );
    p.triangle(
        [cx - TAIL_W / 2.0 + BORDER * 1.5, box_h - BORDER],
        [cx + TAIL_W / 2.0 - BORDER * 1.5, box_h - BORDER],
        [cx, box_h + TAIL_H - BORDER * 2.5],
        WHITE,
    );
    for (i, line) in lines.iter().enumerate() {
        p.text(
            BUBBLE_FONT,
            BLACK,
            line,
            box_w / 2.0,
            PADDING_Y + LINE_H * (i as f32 + 0.5),
        );
    }
}

/// A set's upload, bound twice: its regions read the world's depth, its
/// markers the regions.
#[derive(Clone, Copy, Default)]
struct Set {
    group: u32,
    region_group: u32,
    upload: u32,
    depth: u32,
    numbers: u32,
}

pub struct MarkerPass {
    built: Built,
    region_built: Built,
    /// The regions' numbers and their depth, the world's size.
    numbers: u32,
    numbers_view: u32,
    regions: u32,
    regions_view: u32,
    regions_size: (u32, u32),
    painter: SheetPainter,
    sampler: u32,
    sheet: u32,
    sheet_view: u32,
    sheet_group: u32,
    levels: u32,
    generation: u32,
    sets: [Set; SETS],
    dirty: SlotSet,
    /// A set's uniform a slot (marker.wgsl `marker_words`).
    uniforms: DrawUniforms,
    frame_group: u32,
    pub view: [f32; 16],
    pub proj: [f32; 16],
    pub px_k: f32,
}

impl MarkerPass {
    /// Its regions' depth's bytes, one f32 a pixel.
    pub fn depth_bytes(&self) -> f64 {
        self.regions_size.0 as f64 * self.regions_size.1 as f64 * 4.0
    }

    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, align: u32) -> MarkerPass {
        let sampler = g.sampler(
            FilterMode::nearest,
            FilterMode::linear,
            MipmapFilterMode::linear,
            AddressMode::clamp_to_edge,
            None,
            1,
        );
        let built = pipelines.build(g, PIPELINE_MARKER, g.canvas());
        let frame_group = g.bind_group(built.layouts[0], &[buffer_entry(0, frame_buffer)]);
        MarkerPass {
            built,
            region_built: pipelines.build(g, PIPELINE_MARKER_REGION, Format::rgba8unorm),
            numbers: 0,
            numbers_view: 0,
            regions: 0,
            regions_view: 0,
            regions_size: (0, 0),
            frame_group,
            painter: SheetPainter::new(g, pipelines),
            sampler,
            sheet: 0,
            sheet_view: 0,
            sheet_group: 0,
            levels: 1,
            generation: u32::MAX,
            sets: [Set::default(); SETS],
            dirty: SlotSet::default(),
            uniforms: DrawUniforms::new(word_bytes(MARKER_FLOATS), align),
            view: [0.0; 16],
            proj: [0.0; 16],
            px_k: 1.0,
        }
    }

    /// A sticker whose text is not drawn yet is left for a later frame.
    /// Whether the sheet changed: made again, or a sticker drawn on it.
    pub fn sync_sheet(&mut self, g: &Gpu, mips: &Mips, sheet: &mut Sheet, text: TextOf) -> bool {
        let flush = sheet.flush(self.generation, &mut self.dirty);
        self.generation = flush.generation;
        if flush.whole {
            if flush.size == 0 {
                return false;
            }
            for id in [self.sheet_group, self.sheet_view, self.sheet] {
                release(id);
            }
            self.levels = mip_levels(flush.size, flush.size);
            self.sheet = g.texture(
                flush.size,
                flush.size,
                1,
                Format::rgba8unorm_srgb,
                self.levels,
                texture_usage::TEXTURE_BINDING
                    | texture_usage::COPY_DST
                    | texture_usage::RENDER_ATTACHMENT,
            );
            self.sheet_view = g.view(self.sheet, false, 0, self.levels, 0, 1);
            self.sheet_group = g.bind_group(
                self.built.layouts[3],
                &[
                    view_entry(0, self.sheet_view),
                    sampler_entry(1, self.sampler),
                ],
            );
        }
        if self.dirty.is_empty() || self.sheet == 0 {
            return flush.whole;
        }
        // Only the slots whose text the host has drawn: one still on its way
        // waits for its landing, with no encoder made for it.
        let mut ready = Vec::new();
        for slot in self.dirty.iter() {
            let Some(paint) = sheet.slot_paint(slot) else {
                continue;
            };
            let mut p = Sketch::default();
            match paint {
                Paint::Pill {
                    words,
                    fill,
                    ink,
                    logical_w,
                } => pill(&mut p, words, *fill, *ink, *logical_w as f32),
                Paint::Point {
                    lines,
                    icon,
                    symbol,
                    fill,
                    ink,
                    logical_w,
                } => point(
                    &mut p,
                    lines,
                    *icon,
                    *symbol,
                    *fill,
                    *ink,
                    *logical_w as f32,
                ),
                Paint::Bubble { lines, box_w } => bubble(&mut p, lines, *box_w as f32),
            }
            match SheetPainter::rasters(&p, text) {
                Some(r) => ready.push((slot, p, r)),
                None => sheet.mark_dirty(slot),
            }
        }
        if ready.is_empty() {
            return flush.whole;
        }
        let encoder = g.encoder();
        let mut made = Vec::new();
        let mut drawn = false;
        for (slot, p, rasters) in ready {
            if self.painter.paint(
                g,
                encoder,
                self.sheet,
                sheet.slot_rect(slot),
                &p,
                rasters,
                &mut made,
            ) {
                drawn = true;
            } else {
                sheet.mark_dirty(slot);
            }
        }
        g.submit(encoder);
        // After the submit: a texture let go before it would be gone from the commands that read it.
        for id in made {
            release(id);
        }
        if drawn {
            mips.build(g, self.sheet, self.levels, 0, 1);
        }
        flush.whole || drawn
    }

    /// The regions' depth at the world's size, and each set's groups and
    /// uniform, made here so drawing makes nothing that outlives its frame.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        depth: u32,
        size: (u32, u32),
        draws: &Draws,
        uniforms: &[f32],
        uploads: &Uploads,
    ) {
        if self.sheet_group == 0 {
            return;
        }
        if self.regions_size != size {
            for id in [
                self.numbers_view,
                self.numbers,
                self.regions_view,
                self.regions,
            ] {
                release(id);
            }
            self.numbers = g.texture(
                size.0,
                size.1,
                1,
                Format::rgba8unorm,
                1,
                texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
            );
            self.numbers_view = g.view(self.numbers, false, 0, 1, 0, 1);
            self.regions = g.texture(
                size.0,
                size.1,
                1,
                Format::depth32float,
                1,
                texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
            );
            self.regions_view = g.view(self.regions, false, 0, 1, 0, 1);
            self.regions_size = size;
        }
        let placed = draws
            .of(PIPELINE_MARKER)
            .filter(|d| d[1] > 0)
            .filter_map(|d| uploads.get(d[0]));
        for (set, (upload, _, _)) in self.sets.iter_mut().zip(placed) {
            if set.upload != upload
                || set.depth != depth
                || set.numbers != self.numbers_view
                || set.group == 0
            {
                let bound = |layout: u32, before: u32| {
                    g.bind_group(layout, &[buffer_entry(0, upload), view_entry(1, before)])
                };
                release(set.group);
                release(set.region_group);
                set.group = bound(self.built.layouts[2], self.numbers_view);
                set.region_group = bound(self.region_built.layouts[2], depth);
                (set.upload, set.depth, set.numbers) = (upload, depth, self.numbers_view);
            }
        }
        // A set's uniform a slot, written each frame, as the eye moves them
        // all; numbered on from the sets before, so no two markers share one.
        let placed = || {
            draws
                .of(PIPELINE_MARKER)
                .filter(|d| d[1] > 0)
                .filter(|d| uploads.get(d[0]).is_some())
        };
        let (view, proj, px_k) = (self.view, self.proj, self.px_k);
        let count = placed().take(SETS).count() as u32;
        let mut before = 0;
        self.uniforms
            .write(g, self.built.layouts[1], count, placed(), true, |buf, d| {
                let mut s = [0.0f32; MARKER_FLOATS];
                s[marker::VIEW..marker::VIEW + 16].copy_from_slice(&view);
                s[marker::PROJ..marker::PROJ + 16].copy_from_slice(&proj);
                s[marker::TRANSLATION..marker::TRANSLATION + 3]
                    .copy_from_slice(&uniform_of(uniforms, d)[..3]);
                s[marker::PX_K] = px_k;
                s[marker::BEFORE] = before as f32;
                buf[..word_bytes(MARKER_FLOATS) as usize].copy_from_slice(bytes(&s));
                before += d[1];
            });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        g: &Gpu,
        encoder: u32,
        color: u32,
        depth: u32,
        draws: &Draws,
        uploads: &Uploads,
        stamp: &mut Stamp,
    ) {
        if self.sheet_group == 0 {
            return;
        }
        let placed = || {
            draws
                .of(PIPELINE_MARKER)
                .filter(|d| d[1] > 0)
                .filter(|d| uploads.get(d[0]).is_some())
        };
        if placed().next().is_none() {
            return;
        }
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        // Cleared to no number, and reversed depth to the far plane.
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.numbers_view,
                Load::Clear,
                self.regions_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.region_built.pipeline);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        for (k, (set, d)) in self.sets.iter().zip(placed()).enumerate() {
            let (group, offset) = slots.at(k as u32);
            g.set_bind_group(pass, 1, group, &[offset]);
            g.set_bind_group(pass, 2, set.region_group, &[]);
            g.draw(pass, 4, d[1], 0, 0);
        }
        g.end(pass);
        let pass = g.pass(
            encoder,
            &pass_desc(color, Load::Load, depth, Load::ReadOnly, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        g.set_bind_group(pass, 3, self.sheet_group, &[]);
        for (k, (set, d)) in self.sets.iter().zip(placed()).enumerate() {
            let (group, offset) = slots.at(k as u32);
            g.set_bind_group(pass, 1, group, &[offset]);
            g.set_bind_group(pass, 2, set.group, &[]);
            g.draw(pass, 4, d[1], 0, 0);
        }
        g.end(pass);
    }
}
