// pxl-render/pass/finish.rs

//! Finishes the canvas from the world's color. The world's passes leave their color toned and
//! blended in linear light (gpu/api.rs `WORLD_COLOR`), and the world's is gathered over
//! frames (pass/resolve.rs). The finish brings it up from the world's pixels to the canvas's
//! and encodes it once, at the frame's vignette (screen_finish.wgsl). It carries the world's
//! depth up to the canvas's size with it (`Surface::screen_depth`). One pass finishes the
//! world onto two targets of one color: the canvas, and the picture kept at the canvas's size
//! (`Surface::picture`). The canvas's texture is a new one each frame, so a frame not drawn
//! whole first shows the picture whole on the canvas, then finishes both targets again only in
//! its damaged tiles (screen_tile.wgsl).

use crate::frame::shader::record::{PIPELINE_SCREEN_FINISH, PIPELINE_SCREEN_SHOW};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, view_entry, word_bytes, Gpu, Group, Load,
    RenderContext, TARGET_CAP,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::surface::Surface;
use crate::gpu::tile::{TileGrid, SIDE};
use crate::spec::span::GpuSpanName;

pub struct Finish {
    /// `pipeline` finishes onto the canvas alone, for a character surface. `picture` finishes
    /// onto the canvas and the picture, for the world.
    pipeline: Pipeline,
    picture: Pipeline,
    /// `tiles` is the uniform the finish draws the tiles by, at the canvas's scale. `every` is
    /// bound in place of the tiles' words for a surface with no tiles, and nothing reads it.
    tiles: u32,
    every: u32,
    /// Shows the picture kept at the canvas's size on the canvas.
    show: Pipeline,
    /// The groups `set_groups` sets: a finish group for each color the finish encodes, which
    /// is each of a world's histories or a character surface's one color; and the show's.
    groups: [Group; 2],
    show_group: Group,
}

impl Finish {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Finish {
        Finish {
            pipeline: pipelines.build(g, PIPELINE_SCREEN_FINISH, g.canvas()),
            picture: pipelines.build_targets(g, PIPELINE_SCREEN_FINISH, g.canvas(), TARGET_CAP),
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

    /// Sets the groups for `s` before the frame's commands, over the frame words in `frame`.
    /// When `s` keeps its picture, as the world does, a group is set over each of its histories
    /// with the screen's `tiles`, and the show's over the picture. Otherwise one group is set
    /// over its color.
    pub fn set_groups(&mut self, g: &Gpu, frame: u32, s: &Surface, tiles: Option<&TileGrid>) {
        let has_picture = s.picture_view != 0;
        let (layout, colors) = if has_picture {
            (self.picture.layouts[0], &s.history_view[..])
        } else {
            (
                self.pipeline.layouts[0],
                core::slice::from_ref(&s.color_view),
            )
        };
        let changed_at = tiles.map_or(self.every, TileGrid::changed_at);
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
        if has_picture {
            let layout = self.show.layouts[0];
            self.show_group
                .set(g, layout, &[view_entry(1, s.picture_view)]);
        }
    }

    /// Finishes `color`, `s`'s world color as it is shown, onto `canvas`, and `s`'s depth into
    /// its screen depth, in a pass of its own whose time is counted in `span`. `color` is one
    /// of the colors `set_groups` set a group over. `tiles` holds the screen's tiles the groups
    /// bind, where `s` has them, and whether only their damaged tiles are finished. When `s`
    /// keeps its picture (`Surface::picture`), the pass writes it as well. When only the
    /// damaged tiles are finished, the picture is first shown whole on `canvas`, so the rest
    /// stands as an earlier frame left it.
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        span: GpuSpanName,
        s: &Surface,
        color: u32,
        canvas: u32,
        tiles: Option<(&TileGrid, bool)>,
    ) {
        let has_picture = s.picture_view != 0;
        let damage = tiles
            .filter(|&(_, partial)| partial && has_picture)
            .map(|(t, _)| t);
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(span);
        // The group of the world history that `color` is. A character surface's one color
        // uses the first group.
        let group = self.groups[usize::from(has_picture && color == s.history_view[1])].id();
        // A tile's quad is grown by one canvas pixel each way, measured in the world's pixels,
        // to cover the world texels that a pixel at its edge reads.
        let grow = s.width as f32 / s.screen.0.max(1) as f32;
        let square_cnt = [s.width, s.height].map(|v| v.div_ceil(SIDE));
        let t = match tiles {
            Some((t, _)) => t.words(grow, damage.map_or(u32::MAX, TileGrid::damage_frame_cnt)),
            None => [
                square_cnt[0],
                square_cnt[1],
                u32::MAX,
                0,
                (s.width as f32).to_bits(),
                (s.height as f32).to_bits(),
                grow.to_bits(),
                (SIDE as f32).to_bits(),
            ],
        };
        g.write_buf(self.tiles, 0, bytes(&t));
        if damage.is_some() {
            let pass = g.pass(
                encoder,
                &pass_desc(canvas, Load::Clear, 0, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, self.show.id);
            g.set_bind_group(pass, 0, self.show_group.id(), &[]);
            g.draw(pass, 3, 1, 0, 0);
            g.end(pass);
        }
        let load = if damage.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let desc = pass_desc(canvas, load, s.screen_depth_view, load, 0.0, stamp.pass());
        let (pipeline, desc) = if has_picture {
            (self.picture.id, desc.second_color(s.picture_view))
        } else {
            (self.pipeline.id, desc)
        };
        let pass = g.pass(encoder, &desc);
        g.set_pipeline(pass, pipeline);
        g.set_bind_group(pass, 0, group, &[]);
        g.draw(pass, 6, square_cnt[0] * square_cnt[1], 0, 0);
        g.end(pass);
    }
}
