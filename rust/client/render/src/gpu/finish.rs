// pxl-render/gpu/finish.rs

//! The canvas from the world's color: what its passes left there, toned and
//! blended linear (gpu/surface.rs `WORLD_COLOR`), the world's gathered over
//! frames (gpu/resolve.rs), brought from the world's pixels up to the
//! canvas's and encoded once (screen_finish.wgsl), at the frame's vignette;
//! and the world's depth carried up to the canvas's size with it
//! (`Surface::screen_depth`). The world's is finished into the picture it
//! keeps at the canvas's size (`Surface::shown`), a region frame's only in
//! its region, and shown whole on the canvas.

use crate::frame::shader::record::{PIPELINE_SCREEN_FINISH, PIPELINE_SCREEN_SHOW};
use crate::gpu::api::{buf_entry, pass_desc, release, view_entry, Gpu, Load, RenderContext};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;

pub struct Finish {
    built: Built,
    /// Its group over `bound`, the frame's words and the color and depth it
    /// reads: made again exactly when one is another.
    group: u32,
    bound: [u32; 3],
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
            bound: [0; 3],
            show: pipelines.build(g, PIPELINE_SCREEN_SHOW, g.canvas()),
            show_group: 0,
            shown: 0,
        }
    }

    pub fn release(self) {
        release(self.group);
        release(self.show_group);
    }

    /// `color`, `s`'s world color as it is shown, onto `canvas` and `s`'s
    /// depth into its screen's, a pass of its own, by the frame words in
    /// `frame`, its time counted in `span`. Where `s` keeps its picture
    /// (`Surface::shown`), into it, only within `within`, the canvas's
    /// `[x, y, width, height]`, where one is given, the rest as an earlier
    /// frame left it; then the picture shown whole on `canvas`.
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
        within: Option<[u32; 4]>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(span);
        let bound = [frame, color, s.depth_view];
        if bound != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    buf_entry(0, frame),
                    view_entry(1, color),
                    view_entry(2, s.depth_view),
                ],
            );
            self.bound = bound;
        }
        let kept = s.shown_view != 0;
        let within = within.filter(|_| kept);
        let load = if within.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let target = if kept { s.shown_view } else { canvas };
        let pass = g.pass(
            encoder,
            &pass_desc(target, load, s.screen_depth_view, load, 0.0, stamp.pass()),
        );
        if let Some(rect) = within {
            g.set_scissor(pass, rect);
        }
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
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
