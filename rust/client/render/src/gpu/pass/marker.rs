// pxl-render/gpu/pass/marker.rs

//! The markers, drawn after the world as billboards in two passes
//! (marker.wgsl). The first pass draws each marker's region into a target of
//! its own, as the marker's number. The second draws a marker only where the
//! number at its anchor is its own, with the world's depth attached read
//! only. The markers' sprites are painted into an atlas, premultiplied so its
//! mips filter without a dark fringe.

use crate::frame::shader::record::PIPELINE_MARKER;
use crate::frame::shader::record::PIPELINE_MARKER_REGION;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::word::{marker, MARKER_FLOATS};
use crate::frame::shelf::{Atlas, Paint, SlotSet};
use crate::gpu::api::{
    buf_entry, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes,
    Format, Gpu, Layout, Load, RenderContext,
};
use crate::gpu::atlas::{AtlasPainter, DisplayList};
use crate::gpu::draw::{DrawStats, Draws};
use crate::gpu::mip::{mip_levels, MipPass};
use crate::gpu::pass::model::DrawUniforms;
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::text::TextOf;
use crate::gpu::upload::Uploads;
use crate::look::marker::ATLAS_SCALE;
use crate::look::marker::{
    bubble_rect_h, point_mark, BLACK, BORDER, BUBBLE_FONT, DOT_BORDER, DOT_CY, DOT_DIAM, DOT_FILL,
    DOT_RING, LINE_H, NOTCH, PADDING_Y, PILL_FONT, PILL_H, PILL_R, POINT_FONT, POINT_GAP,
    POINT_LINE, POINT_PAD, POINT_PITCH, POINT_RIM, POINT_RIM_FILL, POINT_SHADE, POINT_SHADOW_BLUR,
    POINT_SHADOW_DY, POINT_SHADOW_FILL, POINT_TEXT_HALO, TAIL_H, TAIL_W, WHITE,
};

const SETS: usize = 32;

fn anchor_dot(p: &mut DisplayList, logical_w: f32) {
    p.circle(logical_w / 2.0, DOT_CY, DOT_DIAM / 2.0, DOT_FILL);
    p.ring(
        logical_w / 2.0,
        DOT_CY,
        DOT_DIAM / 2.0,
        DOT_BORDER,
        DOT_RING,
    );
}

fn point(
    p: &mut DisplayList,
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
        1.0 / ATLAS_SCALE as f32,
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

fn pill(p: &mut DisplayList, text: &str, fill: u32, ink: u32, logical_w: f32) {
    p.round_rect(0.0, 0.0, logical_w, PILL_H, PILL_R, fill);
    p.text(PILL_FONT, ink, text, logical_w / 2.0, PILL_H / 2.0 + 1.0);
    anchor_dot(p, logical_w);
}

/// Fills a rect as two overlapping rects, each inset by `notch` along one
/// axis. The corners they leave uncovered read as a pixel-art rounded corner.
fn notched_rect(p: &mut DisplayList, x: f32, y: f32, w: f32, h: f32, notch: f32, fill: u32) {
    p.rect(x + notch, y, (w - notch * 2.0).max(0.0), h, fill);
    p.rect(x, y + notch, w, (h - notch * 2.0).max(0.0), fill);
}

fn bubble(p: &mut DisplayList, lines: &[String], box_w: f32) {
    let box_h = bubble_rect_h(lines.len());
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

/// A marker set's upload, bound in two groups. The regions' group binds it
/// with the world's depth, and the markers' group binds it with the regions'
/// numbers. The other fields name what the groups were made over.
#[derive(Clone, Copy, Default)]
struct Set {
    group: u32,
    region_group: u32,
    upload: u32,
    depth: u32,
    numbers: u32,
}

pub struct MarkerPass {
    pipeline: Pipeline,
    region_pipeline: Pipeline,
    /// The target the regions draw their markers' numbers into, and its depth
    /// target (`regions`), both at the canvas's size.
    numbers: u32,
    numbers_view: u32,
    regions: u32,
    regions_view: u32,
    regions_size: (u32, u32),
    painter: AtlasPainter,
    sampler: u32,
    atlas: u32,
    atlas_view: u32,
    atlas_group: u32,
    levels: u32,
    generation: u32,
    sets: [Set; SETS],
    dirty: SlotSet,
    /// One uniform slot a set, as marker.wgsl reads `marker_words`.
    uniforms: DrawUniforms,
    frame_group: u32,
    pub view: [f32; 16],
    pub proj: [f32; 16],
    pub px_per_unit: f32,
}

impl MarkerPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buf: u32, align: u32) -> MarkerPass {
        let sampler = g.sampler(
            FilterMode::nearest,
            FilterMode::linear,
            MipmapFilterMode::linear,
            AddressMode::clamp_to_edge,
            None,
            1,
        );
        let pipeline = pipelines.build(g, PIPELINE_MARKER, g.canvas());
        let frame_group = g.bind_group(pipeline.layouts[0], &[buf_entry(0, frame_buf)]);
        MarkerPass {
            pipeline,
            region_pipeline: pipelines.build(g, PIPELINE_MARKER_REGION, Format::rgba8unorm),
            numbers: 0,
            numbers_view: 0,
            regions: 0,
            regions_view: 0,
            regions_size: (0, 0),
            frame_group,
            painter: AtlasPainter::new(g, pipelines),
            sampler,
            atlas: 0,
            atlas_view: 0,
            atlas_group: 0,
            levels: 1,
            generation: u32::MAX,
            sets: [Set::default(); SETS],
            dirty: SlotSet::default(),
            uniforms: DrawUniforms::new(word_bytes(MARKER_FLOATS), align),
            view: [0.0; 16],
            proj: [0.0; 16],
            px_per_unit: 1.0,
        }
    }

    /// Paints the sprites that changed into the atlas, and says whether the
    /// atlas changed: made again, or painted on. A sprite that cannot be
    /// painted yet is left for a later frame: its text is not drawn yet, or
    /// its pixels do not fit this turn's stream.
    pub fn sync_atlas(&mut self, g: &Gpu, mips: &MipPass, atlas: &mut Atlas, text: TextOf) -> bool {
        let flush = atlas.flush(self.generation, &mut self.dirty);
        self.generation = flush.generation;
        if flush.whole {
            if flush.size == 0 {
                return false;
            }
            for id in [self.atlas_group, self.atlas_view, self.atlas] {
                release(id);
            }
            self.levels = mip_levels(flush.size, flush.size);
            self.atlas = g.texture(
                flush.size,
                flush.size,
                1,
                Format::rgba8unorm_srgb,
                self.levels,
                texture_usage::TEXTURE_BINDING
                    | texture_usage::COPY_DST
                    | texture_usage::RENDER_ATTACHMENT,
            );
            self.atlas_view = g.view(self.atlas, false, 0, self.levels, 0, 1);
            self.atlas_group = g.bind_group(
                self.pipeline.layouts[3],
                &[
                    view_entry(0, self.atlas_view),
                    sampler_entry(1, self.sampler),
                ],
            );
        }
        if self.dirty.is_empty() || self.atlas == 0 {
            return flush.whole;
        }
        // Only the slots whose text the driver has drawn are painted. A slot
        // whose text is still on its way waits for it to land, and no encoder
        // is made for it.
        let mut ready = Vec::new();
        for slot in self.dirty.iter() {
            let Some(paint) = atlas.slot_paint(slot) else {
                continue;
            };
            let mut p = DisplayList::default();
            match paint {
                Paint::Pill {
                    text,
                    fill,
                    ink,
                    logical_w,
                } => pill(&mut p, text, *fill, *ink, *logical_w as f32),
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
            match AtlasPainter::rasters(&p, text) {
                Some(r) => ready.push((slot, p, r)),
                None => atlas.mark_dirty(slot),
            }
        }
        if ready.is_empty() {
            return flush.whole;
        }
        let encoder = g.encoder();
        let mut made = Vec::new();
        let mut drawn = false;
        for (slot, p, rasters) in ready {
            let painted =
                self.painter
                    .paint(g, encoder, self.atlas, atlas.slot_rect(slot), &p, rasters);
            match painted {
                Some(ids) => {
                    made.extend(ids);
                    drawn = true;
                }
                None => atlas.mark_dirty(slot),
            }
        }
        g.submit(encoder);
        // What the painter made is released only after the submit. A texture
        // released before it would be gone from the commands that read it.
        for id in made {
            release(id);
        }
        if drawn {
            mips.build(g, self.atlas, self.levels, 0, 1);
        }
        flush.whole || drawn
    }

    /// Makes the regions' targets at the canvas's size, and each set's groups
    /// and uniform, so that drawing makes none: a group made inside a frame
    /// that is dropped would be gone.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        depth: u32,
        size: (u32, u32),
        draws: &Draws,
        uploads: &Uploads,
    ) {
        if self.atlas_group == 0 {
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
                let bound = |layout: Layout, before: u32| {
                    g.bind_group(layout, &[buf_entry(0, upload), view_entry(1, before)])
                };
                release(set.group);
                release(set.region_group);
                set.group = bound(self.pipeline.layouts[2], self.numbers_view);
                set.region_group = bound(self.region_pipeline.layouts[2], depth);
                (set.upload, set.depth, set.numbers) = (upload, depth, self.numbers_view);
            }
        }
        // Each set's uniform has a slot and is written every frame, because the
        // eye's view moves every set. A set numbers its markers on from the
        // sets before it (`marker::BEFORE`), so no two markers share a number.
        let placed = || {
            draws
                .of(PIPELINE_MARKER)
                .filter(|d| d[1] > 0)
                .filter(|d| uploads.get(d[0]).is_some())
        };
        let (view, proj, px_per_unit) = (self.view, self.proj, self.px_per_unit);
        let cnt = placed().take(SETS).count() as u32;
        let mut before = 0;
        self.uniforms.write(
            g,
            self.pipeline.layouts[1],
            cnt,
            placed(),
            true,
            |buf, d| {
                let mut s = [0.0f32; MARKER_FLOATS];
                s[marker::VIEW..marker::VIEW + 16].copy_from_slice(&view);
                s[marker::PROJ..marker::PROJ + 16].copy_from_slice(&proj);
                s[marker::PX_PER_UNIT] = px_per_unit;
                s[marker::BEFORE] = before as f32;
                buf[..word_bytes(MARKER_FLOATS) as usize].copy_from_slice(bytes(&s));
                before += d[1];
            },
        );
    }

    /// Adds the pass's draws to `stats`, two triangles a marker.
    pub fn count(&self, draws: &Draws, stats: &mut DrawStats) {
        draws.count(PIPELINE_MARKER, stats, |d| 2 * d[1]);
    }

    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        color: u32,
        depth: u32,
        draws: &Draws,
        uploads: &Uploads,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::marker);
        if self.atlas_group == 0 {
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
        // The numbers are cleared to 0, which is no marker, and the reversed
        // depth to 0, the far plane.
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
        g.set_pipeline(pass, self.region_pipeline.id);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        for (k, (set, d)) in self.sets.iter().zip(placed()).enumerate() {
            let (group, off) = slots.at(k as u32);
            g.set_bind_group(pass, 1, group, &[off]);
            g.set_bind_group(pass, 2, set.region_group, &[]);
            g.draw(pass, 4, d[1], 0, 0);
        }
        g.end(pass);
        let pass = g.pass(
            encoder,
            &pass_desc(color, Load::Load, depth, Load::ReadOnly, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.pipeline.id);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        g.set_bind_group(pass, 3, self.atlas_group, &[]);
        for (k, (set, d)) in self.sets.iter().zip(placed()).enumerate() {
            let (group, off) = slots.at(k as u32);
            g.set_bind_group(pass, 1, group, &[off]);
            g.set_bind_group(pass, 2, set.group, &[]);
            g.draw(pass, 4, d[1], 0, 0);
        }
        g.end(pass);
    }
}
