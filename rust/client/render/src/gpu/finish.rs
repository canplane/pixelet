// pxl-render/gpu/finish.rs

//! The canvas from the world's color: what its passes left there, toned and
//! blended linear (gpu/surface.rs `WORLD_COLOR`), the world's gathered over
//! frames (gpu/resolve.rs), brought from the world's pixels up to the
//! canvas's and encoded once (screen_finish.wgsl), at the frame's vignette;
//! and the world's depth carried up to the canvas's size with it
//! (`Surface::screen_depth`). The world's is finished into the picture it
//! keeps at the canvas's size (`Surface::shown`), a frame not drawn whole's
//! only in its claimed tiles (screen_tiles.wgsl), and shown whole on the
//! canvas.

use crate::frame::shader::record::{PIPELINE_SCREEN_FINISH, PIPELINE_SCREEN_SHOW};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, view_entry, word_bytes, Gpu, Load,
    RenderContext,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;
use crate::gpu::tiles::{Tiles, SIDE};

pub struct Finish {
    built: Built,
    /// Its group over `bound`, the frame's words, the color and depth it
    /// reads and the tiles' words: made again exactly when one is another.
    group: u32,
    bound: [u32; 4],
    /// The tiles' uniform it draws them by, at the canvas's scale, and the
    /// words it binds where it draws every tile, which none reads.
    tiles: u32,
    every: u32,
    /// The kept picture shown on the canvas, its group over the picture.
    show: Built,
    show_group: u32,
    shown: u32,
}

impl Finish {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Finish {
        Finish {
            built: pipelines.build(g, PIPELINE_SCREEN_FINISH, g.canvas()),
            group: 0,
            bound: [0; 4],
            tiles: g.buf(
                word_bytes(8),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            every: g.buf(word_bytes(1), buffer_usage::STORAGE),
            show: pipelines.build(g, PIPELINE_SCREEN_SHOW, g.canvas()),
            show_group: 0,
            shown: 0,
        }
    }

    pub fn release(self) {
        release(self.group);
        release(self.show_group);
        release(self.tiles);
        release(self.every);
    }

    /// `color`, `s`'s world color as it is shown, onto `canvas` and `s`'s
    /// depth into its screen's, a pass of its own, by the frame words in
    /// `frame`, its time counted in `span`. Where `s` keeps its picture
    /// (`Surface::shown`), into it, only in the claimed tiles of the words
    /// `claimed` names (`Tiles`) where it names any, the rest as an
    /// earlier frame left it; then the picture shown whole on `canvas`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pass's targets, each its own"
    )]
    pub fn draw(
        &mut self,
        ctx: &mut RenderContext,
        span: GpuSpanName,
        frame: u32,
        s: &Surface,
        color: u32,
        canvas: u32,
        claimed: Option<&Tiles>,
    ) {
        let kept = s.shown_view != 0;
        let claimed = claimed.filter(|_| kept);
        let (changed_at, tiles) = claimed.map_or(
            (self.every, [s.width, s.height].map(|v| v.div_ceil(SIDE))),
            |t| (t.changed_at(), t.squares()),
        );
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(span);
        let bound = [frame, color, s.depth_only_view, changed_at];
        if bound != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    buf_entry(0, frame),
                    view_entry(1, color),
                    view_entry(2, s.depth_only_view),
                    buf_entry(30, changed_at),
                    buf_entry(31, self.tiles),
                ],
            );
            self.bound = bound;
        }
        let partial = claimed.is_some();
        // A tile's quad grown by a canvas pixel each way, in the world's
        // pixels, for the world texels a pixel at its edge reads.
        let grow = s.width as f32 / s.screen.0.max(1) as f32;
        let size = [s.width, s.height];
        let t = claimed.map_or(
            [
                tiles[0],
                tiles[1],
                u32::MAX,
                0,
                (s.width as f32).to_bits(),
                (s.height as f32).to_bits(),
                grow.to_bits(),
                (SIDE as f32).to_bits(),
            ],
            |c| c.words(size, grow, c.claimed_for()),
        );
        g.write_buf(self.tiles, 0, bytes(&t));
        let load = if partial { Load::Load } else { Load::Clear };
        let target = if kept { s.shown_view } else { canvas };
        let pass = g.pass(
            encoder,
            &pass_desc(target, load, s.screen_depth_view, load, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 6, tiles[0] * tiles[1], 0, 0);
        g.end(pass);
        if !kept {
            return;
        }
        if s.shown_view != self.shown {
            release(self.show_group);
            self.show_group = g.bind_group(self.show.layouts[0], &[view_entry(1, s.shown_view)]);
            self.shown = s.shown_view;
        }
        let pass = g.pass(
            encoder,
            &pass_desc(canvas, Load::Clear, 0, Load::Clear, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.show.pipeline);
        g.set_bind_group(pass, 0, self.show_group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
    }
}
