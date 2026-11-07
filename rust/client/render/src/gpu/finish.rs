// pxl-render/gpu/finish.rs

//! The canvas from the world's color: what its passes left there, toned and
//! blended linear (gpu/surface.rs `WORLD_COLOR`), the world's gathered over
//! frames (gpu/resolve.rs), brought from the world's pixels up to the
//! canvas's and encoded once (screen_finish.wgsl), at the frame's vignette;
//! and the world's depth carried up to the canvas's size with it
//! (`Surface::screen_depth`). The world's is finished in one pass onto the
//! canvas and into the picture it keeps at the canvas's size
//! (`Surface::picture`), two targets of one color. A frame not drawn whole
//! shows the picture whole on the canvas first, whose texture is a new one
//! each frame, then finishes both again only in its claimed tiles
//! (screen_tiles.wgsl).

use crate::frame::shader::record::{PIPELINE_SCREEN_FINISH, PIPELINE_SCREEN_SHOW};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, view_entry, word_bytes, Gpu, Group, Load,
    RenderContext, TARGETS,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;
use crate::gpu::tiles::{Tiles, SIDE};

pub struct Finish {
    /// Onto the canvas alone, a character surface's; and onto the canvas and the
    /// picture, the world's.
    pipeline: Pipeline,
    kept: Pipeline,
    /// The tiles' uniform it draws them by, at the canvas's scale, and the
    /// words it binds where it draws a surface with no tiles, which none
    /// reads.
    tiles: u32,
    every: u32,
    /// The picture at the canvas's size shown on the canvas.
    show: Pipeline,
    /// Its groups (`set_groups`): the finish's of each color it encodes, a
    /// world's history each or a character surface's color, and the show's.
    groups: [Group; 2],
    show_group: Group,
}

impl Finish {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Finish {
        Finish {
            pipeline: pipelines.build(g, PIPELINE_SCREEN_FINISH, g.canvas()),
            kept: pipelines.build_targets(g, PIPELINE_SCREEN_FINISH, g.canvas(), TARGETS),
            tiles: g.buf(
                word_bytes(8),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            every: g.buf(word_bytes(1), buffer_usage::STORAGE),
            show: pipelines.build(g, PIPELINE_SCREEN_SHOW, g.canvas()),
            groups: [Group::default(), Group::default()],
            show_group: Group::default(),
        }
    }

    pub fn release(mut self) {
        release(self.tiles);
        release(self.every);
        self.groups.iter_mut().for_each(Group::release);
        self.show_group.release();
    }

    /// Its groups for `s`, before the frame's commands, by the frame words
    /// in `frame`: where `s` keeps its picture, the world's, over each of its
    /// histories, the screen's tiles `tiles` and the picture; else over its
    /// color alone.
    pub fn set_groups(&mut self, g: &Gpu, frame: u32, s: &Surface, tiles: Option<&Tiles>) {
        let kept = s.picture_view != 0;
        let (layout, colors) = if kept {
            (self.kept.layouts[0], &s.history_view[..])
        } else {
            (
                self.pipeline.layouts[0],
                core::slice::from_ref(&s.color_view),
            )
        };
        let changed_at = tiles.map_or(self.every, Tiles::changed_at);
        for (group, &color) in self.groups.iter_mut().zip(colors) {
            group.set(
                g,
                layout,
                &[
                    buf_entry(0, frame),
                    view_entry(1, color),
                    view_entry(2, s.depth_only_view),
                    buf_entry(30, changed_at),
                    buf_entry(31, self.tiles),
                ],
            );
        }
        if kept {
            let layout = self.show.layouts[0];
            self.show_group
                .set(g, layout, &[view_entry(1, s.picture_view)]);
        }
    }

    /// `color`, `s`'s world color as it is shown, one its groups were set
    /// over (`set_groups`), onto `canvas` and `s`'s depth into its screen's,
    /// a pass of its own, its time counted in `span`, by `tiles`, the
    /// screen's tiles the groups bind (`Tiles`) where `s` has them, and
    /// whether only their claimed tiles are finished. Where `s` keeps its
    /// picture (`Surface::picture`), into it as well; and where only the
    /// claimed are, the picture shown whole on `canvas` first, the rest as
    /// an earlier frame left it.
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        span: GpuSpanName,
        s: &Surface,
        color: u32,
        canvas: u32,
        tiles: Option<(&Tiles, bool)>,
    ) {
        let kept = s.picture_view != 0;
        let claimed = tiles
            .filter(|&(_, partial)| partial && kept)
            .map(|(t, _)| t);
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(span);
        // The group of the history `color` is, a world's; a character surface's one
        // color has the first.
        let group = self.groups[usize::from(kept && color == s.history_view[1])].id();
        // A tile's quad grown by a canvas pixel each way, in the world's
        // pixels, for the world texels a pixel at its edge reads.
        let grow = s.width as f32 / s.screen.0.max(1) as f32;
        let squares = [s.width, s.height].map(|v| v.div_ceil(SIDE));
        let t = match tiles {
            Some((t, _)) => t.words(grow, claimed.map_or(u32::MAX, Tiles::claimed_for)),
            None => [
                squares[0],
                squares[1],
                u32::MAX,
                0,
                (s.width as f32).to_bits(),
                (s.height as f32).to_bits(),
                grow.to_bits(),
                (SIDE as f32).to_bits(),
            ],
        };
        g.write_buf(self.tiles, 0, bytes(&t));
        if claimed.is_some() {
            let pass = g.pass(
                encoder,
                &pass_desc(canvas, Load::Clear, 0, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, self.show.id);
            g.set_bind_group(pass, 0, self.show_group.id(), &[]);
            g.draw(pass, 3, 1, 0, 0);
            g.end(pass);
        }
        let load = if claimed.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let desc = pass_desc(canvas, load, s.screen_depth_view, load, 0.0, stamp.pass());
        let (pipeline, desc) = if kept {
            (self.kept.id, desc.second_color(s.picture_view))
        } else {
            (self.pipeline.id, desc)
        };
        let pass = g.pass(encoder, &desc);
        g.set_pipeline(pass, pipeline);
        g.set_bind_group(pass, 0, group, &[]);
        g.draw(pass, 6, squares[0] * squares[1], 0, 0);
        g.end(pass);
    }
}
