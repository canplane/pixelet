// pxl-render/gpu/pass/character.rs

//! A character surface: the companion's character in a frame of its
//! own, over the flat color the UI asks for, drawn at one sample with no
//! shadow, a blank map bound and no strength to read it by. Its draw is
//! handed to it whole, held by none of the world's lists, so what the
//! world's eye sees of the character moves nothing here: its uniform at
//! slot 0 of a buffer of its own (gpu/draw.rs `scaled_draw`).

use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::v3::V3;

use crate::frame::shader::word::frame::{
    CAMERA_POS, DRIFT_Q, EYE, JITTER, SCREEN, SHADOW_STRENGTH, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::{DRAW_FLOATS, FRAME_FLOATS};
use crate::gpu::api::{
    buffer_usage, bytes, pass_desc, release, word_bytes, Entry, Gpu, Image, Load, RenderContext,
    TrackedRenderPass,
};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::draw::{Draw, Position};
use crate::gpu::finish::Finish;
use crate::gpu::pass::model::{frame_layout, FrameGroups, Stage};
use crate::gpu::pass::skinned_mesh::SkinnedPass;
use crate::gpu::pipeline::Pipelines;
use crate::gpu::put::put_mat;
use crate::gpu::read::Read;
use crate::gpu::shadow::Shadow;
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;
use crate::gpu::upload::uploads;
use crate::look::color::linear_rgb;

pub struct CharacterPass {
    frame_buf: u32,
    /// Its draw's uniform, the one slot its skinned pass binds.
    draw_buf: u32,
    frames: FrameGroups,
    pub skinned_mesh: SkinnedPass,
    finish: Finish,
    /// What its picture is made of as last drawn: its frame words, its
    /// draw, its skin and its surface; it stands, and its surface keeps its
    /// picture, while they do.
    pub(crate) read: Read,
}

const CHARACTER_EYE: V3 = [0.0, -2.2, 0.9];
const CHARACTER_TARGET: V3 = [0.0, 0.0, 0.55];
const CHARACTER_FOV_DEG: f64 = 35.0;
/// The character surface's near plane: a room of its own, a model a unit tall in it.
const CHARACTER_NEAR: f64 = 0.1;

/// What the world's color is cleared to for `bg` (0xRRGGBB, sRGB): linear, as
/// the finish encodes it back, and whole.
fn clear_of(bg: u32) -> [f32; 4] {
    let [r, g, b] = linear_rgb(bg).map(|c| c as f32);
    [r, g, b, 1.0]
}

impl CharacterPass {
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        shadow: &Shadow,
        air: [Entry; AIR_ENTRIES],
        align: u32,
    ) -> CharacterPass {
        let frame_buf = g.buf(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let draw_buf = g.buf(
            word_bytes(DRAW_FLOATS),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let layout = frame_layout(g, pipelines);
        let mut skinned_mesh = SkinnedPass::new(g, pipelines, align);
        skinned_mesh.set_draws(g, draw_buf);
        CharacterPass {
            frame_buf,
            draw_buf,
            frames: FrameGroups::new(g, layout, frame_buf, shadow.blank_entries(1), air),
            skinned_mesh,
            finish: Finish::new(g, pipelines),
            read: Read::default(),
        }
    }

    /// Its finish's groups over `s`, before the frame's commands, `s` made
    /// at its size.
    pub fn set_groups(&mut self, g: &Gpu, s: &Surface) {
        self.finish.set_groups(g, self.frame_buf, s, None);
    }

    pub fn release(self) {
        self.frames.release();
        release(self.frame_buf);
        release(self.draw_buf);
        self.skinned_mesh.release();
        self.finish.release();
    }

    /// `draw`, the character's, into `s` over `bg` (0xRRGGBB, sRGB), the
    /// world's `frame` aimed at the character; nothing where its picture's
    /// makings stand, `moved` what moved without words.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is what its picture is made of, which `read` reads apart to tell whether it stands"
    )]
    pub fn draw(
        &mut self,
        ctx: &mut RenderContext,
        s: &Surface,
        mut frame: [f32; FRAME_FLOATS],
        (record, uniform): &Draw,
        image: Image,
        bg: u32,
        moved: bool,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::character);
        // Its eye stands at the character's Q3, so the character is its
        // remainder from it.
        let at = Position::in_uniform(uniform);
        let character = at.r.map(f64::from);
        let u = &mut frame;
        for k in 0..3 {
            u[EYE + k] = f32::from_bits(at.q[k] as u32);
        }
        let (mut proj, mut view, mut t, mut m, mut n) = (
            mat4::new(),
            mat4::new(),
            mat4::new(),
            mat4::new(),
            mat4::new(),
        );
        mat4::perspective(
            &mut proj,
            CHARACTER_FOV_DEG.to_radians(),
            s.width as f64 / s.height as f64,
            CHARACTER_NEAR,
        );
        mat4::look_at(&mut view, CHARACTER_EYE, CHARACTER_TARGET, [0.0, 0.0, 1.0]);
        mat4::translation(&mut t, -character[0], -character[1], -character[2]);
        mat4::mul(&mut n, &proj, &view);
        mat4::mul(&mut m, &n, &t);
        put_mat(u, VIEW_PROJ, &m);
        u[CAMERA_POS] = (CHARACTER_EYE[0] + character[0]) as f32;
        u[CAMERA_POS + 1] = (CHARACTER_EYE[1] + character[1]) as f32;
        u[CAMERA_POS + 2] = (CHARACTER_EYE[2] + character[2]) as f32;
        u[VIEWPORT] = s.width as f32;
        u[VIEWPORT + 1] = s.height as f32;
        u[SCREEN] = s.screen.0 as f32;
        u[SCREEN + 1] = s.screen.1 as f32;
        u[VIGNETTE] = 0.0;
        u[SHADOW_STRENGTH] = 0.0;
        // Drawn at one sample a pixel, its frames gathered by none.
        u[JITTER] = 0.0;
        u[JITTER + 1] = 0.0;
        // A character drifts with no wind.
        u[DRIFT_Q..DRIFT_Q + 4].fill(0.0);
        let r = &mut self.read;
        r.begin();
        r.read(&frame);
        r.read(record);
        r.read(uniform);
        r.read(&[image.0, s.width, s.height, s.depth, bg]);
        r.mark(moved);
        if r.end() {
            return;
        }
        g.write_buf(self.frame_buf, 0, bytes(&frame));
        g.write_buf(self.draw_buf, 0, bytes(uniform));
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.color_view,
                Load::Clear,
                s.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            )
            .color_clear(clear_of(bg)),
        );
        let mut pass = TrackedRenderPass::new(g, pass);
        self.skinned_mesh.draw(
            &mut pass,
            core::iter::once(record),
            uploads(),
            Stage::Lit,
            self.frames.frame,
        );
        pass.end();
        // Encoded onto its canvas at its own frame's words, no vignette.
        let view_id = g.cur(s.handle);
        self.finish
            .draw(ctx, GpuSpanName::character, s, s.color_view, view_id, None);
        release(view_id);
    }
}

#[cfg(test)]
#[path = "character.test.rs"]
mod test;
