// pxl-render/gpu/pass/case.rs

//! A variant picker's display case: its actor in a frame of its own, drawn at
//! one sample with no shadow, a blank map bound and no strength to read it by.

use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::v3::V3;

use crate::frame::draw::{uniform_of, DRAW_WORDS};
use crate::frame::shader::word::frame::{
    CAMERA_POS, SCREEN, SCREEN_AO, SHADOW_STRENGTH, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::FRAME_FLOATS;
use crate::gpu::ao::{ScreenAo, MAP_BINDING};
use crate::gpu::api::{
    buffer_usage, bytes, pass_desc, release, word_bytes, Entry, Gpu, Image, Load, Stamp,
};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::finish::Finish;
use crate::gpu::pass::model::{frame_layout, FrameGroups, Stage};
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::Pipelines;
use crate::gpu::put::put_mat;
use crate::gpu::read::Read;
use crate::gpu::shadow::Shadow;
use crate::gpu::surface::Surface;
use crate::gpu::upload::uploads;

pub struct Case {
    frame_buffer: u32,
    frames: FrameGroups,
    pub skinned: SkinnedPass,
    finish: Finish,
    /// What its picture is made of as last drawn: its frame words, its
    /// actor's draw and uniform, its skin and its surface; it stands, and
    /// its surface keeps its picture, while they do.
    pub(crate) read: Read,
}

const CASE_EYE: V3 = [0.0, -2.2, 0.9];
const CASE_TARGET: V3 = [0.0, 0.0, 0.55];
const CASE_FOV_DEG: f64 = 35.0;
/// The case's near plane: a room of its own, a model a unit tall in it.
const CASE_NEAR: f64 = 0.1;

impl Case {
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        shadow: &Shadow,
        ao: &ScreenAo,
        air: [Entry; AIR_ENTRIES],
        align: u32,
    ) -> Case {
        let frame_buffer = g.buffer(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let layout = frame_layout(g, pipelines);
        Case {
            frame_buffer,
            frames: FrameGroups::new(
                g,
                layout,
                frame_buffer,
                shadow.blank_entries(1),
                [ao.blank_entry(MAP_BINDING); 2],
                air,
            ),
            skinned: SkinnedPass::new(g, pipelines, align),
            finish: Finish::new(g, pipelines),
            read: Read::default(),
        }
    }

    pub fn release(self) {
        self.frames.release();
        release(self.frame_buffer);
        self.skinned.release();
        self.finish.release();
    }

    /// Its actor's draw `own` into `s`, the world's `frame` aimed at the
    /// actor; nothing where its picture's makings stand, `moved` what moved
    /// without words.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        g: &Gpu,
        encoder: u32,
        s: &Surface,
        mut frame: [f32; FRAME_FLOATS],
        own: &[u32; DRAW_WORDS],
        uniforms: &[f32],
        model: u32,
        image: Image,
        moved: bool,
        stamp: &mut Stamp,
    ) {
        let u = uniform_of(uniforms, own);
        let actor = [u[0] as f64, u[1] as f64, u[2] as f64];
        let u = &mut frame;
        let (mut proj, mut view, mut t, mut m, mut n) = (
            mat4::new(),
            mat4::new(),
            mat4::new(),
            mat4::new(),
            mat4::new(),
        );
        mat4::perspective(
            &mut proj,
            CASE_FOV_DEG.to_radians(),
            s.width as f64 / s.height as f64,
            CASE_NEAR,
        );
        mat4::look_at(&mut view, CASE_EYE, CASE_TARGET, [0.0, 0.0, 1.0]);
        mat4::translation(&mut t, -actor[0], -actor[1], -actor[2]);
        mat4::mul(&mut n, &proj, &view);
        mat4::mul(&mut m, &n, &t);
        put_mat(u, VIEW_PROJ, &m);
        u[CAMERA_POS] = (CASE_EYE[0] + actor[0]) as f32;
        u[CAMERA_POS + 1] = (CASE_EYE[1] + actor[1]) as f32;
        u[CAMERA_POS + 2] = (CASE_EYE[2] + actor[2]) as f32;
        u[VIEWPORT] = s.width as f32;
        u[VIEWPORT + 1] = s.height as f32;
        u[SCREEN] = s.screen.0 as f32;
        u[SCREEN + 1] = s.screen.1 as f32;
        u[VIGNETTE] = 0.0;
        u[SCREEN_AO] = 0.0;
        u[SHADOW_STRENGTH] = 0.0;
        let r = &mut self.read;
        r.begin();
        r.read(&frame);
        r.read(own);
        r.read(uniform_of(uniforms, own));
        r.read(&[model, image.0, s.width, s.height, s.depth]);
        r.mark(moved);
        if r.end() {
            return;
        }
        g.write_buffer(self.frame_buffer, 0, bytes(&frame));
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.color_view,
                Load::Clear,
                s.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        self.skinned.draw(
            g,
            pass,
            core::iter::once(own),
            uploads(),
            Stage::Lit,
            (self.frames.frame, &mut 0),
        );
        g.end(pass);
        // Encoded onto its canvas at its own frame's words, no vignette.
        let view_id = g.current(s.handle);
        self.finish
            .draw(g, encoder, self.frame_buffer, s, view_id, stamp);
        release(view_id);
    }
}
