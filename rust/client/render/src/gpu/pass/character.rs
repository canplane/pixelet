// pxl-render/gpu/pass/character.rs

//! A character surface shows the companion's character alone, seen from a
//! camera of its own, over the flat color the UI asks for. It draws one
//! sample a pixel and no shadow: a blank shadow map is bound, and the
//! frame's shadow strength is zero. The caller hands the pass the
//! character's whole draw, which none of the world's draw lists holds, so
//! what the world's camera sees of the character changes nothing here. The
//! draw's record names uniform slot 0 (gpu/draw.rs `scaled_draw`), and the
//! pass keeps the uniform there in a buffer of its own.

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
use crate::gpu::pass::model::{frame_layout, FrameGroup, Stage};
use crate::gpu::pass::skinned_mesh::SkinnedPass;
use crate::gpu::pipeline::PipelineCache;
use crate::gpu::put::put_mat;
use crate::gpu::read::Read;
use crate::gpu::shadow::Shadow;
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;
use crate::gpu::upload::uploads;
use crate::look::color::linear_rgb;

pub struct CharacterPass {
    frame_buf: u32,
    /// The buffer the skinned pass reads its draw uniforms from here. It has
    /// one slot, which holds the character's.
    draw_buf: u32,
    frames: FrameGroup,
    pub skinned_mesh: SkinnedPass,
    finish: Finish,
    /// What the surface's picture was made of when it was last drawn: the
    /// frame words, the draw, the skin image and the surface. While all of
    /// them read the same, the pass draws nothing and the surface keeps its
    /// picture.
    pub(crate) read: Read,
}

const CHARACTER_EYE: V3 = [0.0, -2.2, 0.9];
const CHARACTER_TARGET: V3 = [0.0, 0.0, 0.55];
const CHARACTER_FOV_DEG: f64 = 35.0;
/// The surface's near plane. Its scene holds nothing but the character, a
/// model about a unit tall, so the plane is set for that scale and not the
/// world's.
const CHARACTER_NEAR: f64 = 0.1;

/// The color the surface's world color is cleared to for `bg`, a 0xRRGGBB
/// sRGB color. The world color is linear and the finish encodes it back to
/// sRGB, so the clear is `bg` made linear. Its alpha is 1, so nothing shows
/// through.
fn clear_of(bg: u32) -> [f32; 4] {
    let [r, g, b] = linear_rgb(bg).map(|c| c as f32);
    [r, g, b, 1.0]
}

impl CharacterPass {
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
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
            frames: FrameGroup::new(g, layout, frame_buf, shadow.blank_entries(1), air),
            skinned_mesh,
            finish: Finish::new(g, pipelines),
            read: Read::default(),
        }
    }

    /// Makes the finish's bind groups over `s`. The caller calls it before
    /// the frame's commands are encoded and after `s` is made at its size.
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

    /// Draws the character's `draw` into `s` over `bg`, a 0xRRGGBB sRGB
    /// color. `frame` holds the world's frame words, which the pass aims at
    /// the character. It draws nothing while all its picture is made of
    /// reads the same as last time. `changed` says that something without
    /// words to read changed, such as a model's upload.
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
        changed: bool,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::character);
        // The frame's eye is set to the character's Q3 (gpu/draw.rs
        // `Position`), so the character sits at its remainder from the eye.
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
        // No resolve gathers this surface's frames, so it draws one sample a
        // pixel with no jitter.
        u[JITTER] = 0.0;
        u[JITTER + 1] = 0.0;
        // The cloud field's drift moves nothing here. Zeroed, it keeps a
        // frame where only the wind moved from reading as changed.
        u[DRIFT_Q..DRIFT_Q + 4].fill(0.0);
        let r = &mut self.read;
        r.begin();
        r.read(&frame);
        r.read(record);
        r.read(uniform);
        r.read(&[image.0, s.width, s.height, s.depth, bg]);
        r.mark(changed);
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
        // The finish encodes the color onto the surface's canvas by this
        // pass's own frame words, whose vignette is zero.
        let view_id = g.cur(s.handle);
        self.finish
            .draw(ctx, GpuSpanName::character, s, s.color_view, view_id, None);
        release(view_id);
    }
}

#[cfg(test)]
#[path = "character.test.rs"]
mod test;
