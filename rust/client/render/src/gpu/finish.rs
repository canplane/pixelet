// pxl-render/gpu/finish.rs

//! The canvas from the world's color: what its passes left there, toned and
//! blended linear (gpu/surface.rs `WORLD_COLOR`), encoded once
//! (screen_finish.wgsl), at the frame's vignette.

use crate::frame::shader::record::PIPELINE_SCREEN_FINISH;
use crate::gpu::api::{buffer_entry, pass_desc, release, view_entry, Gpu, Load, Stamp};
use crate::gpu::pipeline::{Built, Pipelines};

pub struct Finish {
    built: Built,
    /// Its group over `bound`, the frame's words and the color it reads:
    /// made again exactly when either is another.
    group: u32,
    bound: [u32; 2],
}

impl Finish {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Finish {
        Finish {
            built: pipelines.build(g, PIPELINE_SCREEN_FINISH, g.canvas()),
            group: 0,
            bound: [0; 2],
        }
    }

    pub fn release(self) {
        release(self.group);
    }

    /// `color` onto `canvas`, a pass of its own, by the frame words in
    /// `frame`, its time counted in `stamp`'s span, the pass it finishes.
    pub fn draw(
        &mut self,
        g: &Gpu,
        encoder: u32,
        frame: u32,
        color: u32,
        canvas: u32,
        stamp: &mut Stamp,
    ) {
        if [frame, color] != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[buffer_entry(0, frame), view_entry(1, color)],
            );
            self.bound = [frame, color];
        }
        let pass = g.pass(
            encoder,
            &pass_desc(canvas, Load::Clear, 0, Load::Clear, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
    }
}
