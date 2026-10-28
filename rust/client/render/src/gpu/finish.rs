// pxl-render/gpu/finish.rs

//! The canvas from the world's color: what its passes left there, toned and
//! blended linear (gpu/surface.rs `WORLD_COLOR`), brought from the world's
//! pixels up to the canvas's and encoded once (screen_finish.wgsl), at the
//! frame's vignette; and the world's depth carried up to the canvas's size
//! with it (`Surface::screen_depth`).

use crate::frame::shader::record::PIPELINE_SCREEN_FINISH;
use crate::gpu::api::{buffer_entry, pass_desc, release, view_entry, Gpu, Load, Stamp};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::surface::Surface;

pub struct Finish {
    built: Built,
    /// Its group over `bound`, the frame's words and the color and depth it
    /// reads: made again exactly when one is another.
    group: u32,
    bound: [u32; 3],
}

impl Finish {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Finish {
        Finish {
            built: pipelines.build(g, PIPELINE_SCREEN_FINISH, g.canvas()),
            group: 0,
            bound: [0; 3],
        }
    }

    pub fn release(self) {
        release(self.group);
    }

    /// `s`'s color onto `canvas` and its depth into its screen's, a pass of
    /// its own, by the frame words in `frame`, its time counted in `stamp`'s
    /// span.
    pub fn draw(
        &mut self,
        g: &Gpu,
        encoder: u32,
        frame: u32,
        s: &Surface,
        canvas: u32,
        stamp: &mut Stamp,
    ) {
        let bound = [frame, s.color_view, s.depth_view];
        if bound != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    buffer_entry(0, frame),
                    view_entry(1, s.color_view),
                    view_entry(2, s.depth_view),
                ],
            );
            self.bound = bound;
        }
        let pass = g.pass(
            encoder,
            &pass_desc(
                canvas,
                Load::Clear,
                s.screen_depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
    }
}
