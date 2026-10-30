// pxl-render/gpu/pass/skinned.rs

//! Actors: each model's mesh upload skinned by each actor's joint palette. A
//! case of the variant picker draws one actor in a pass of its own. An actor
//! whose draw changed marks the screen's tiles it covers where it was and
//! where it is (`draw_marks`, tile_mark.wgsl): what it was drawn with last
//! frame kept, its record, its uniform and its palette copied (`keep`).

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_MARK};

use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, Format, Gpu, Image, Stencil, TrackedRenderPass,
};
use crate::gpu::pass::model::{
    draw_model, draw_models, skin_group, skin_made, DrawUniforms, ModelDraw, Stage, DRAW_BYTES,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::Uploads;

/// An actor's draw as a frame drew it: its record and its uniform's first
/// words (skinned.wgsl `Draw`), which name its place.
type Drawn = ([u32; DRAW_WORDS], [f32; 6]);

pub struct SkinnedPass {
    lit: Built,
    depth: Built,
    uniforms: DrawUniforms,
    /// Each model's skin image, as the frame hands them.
    skins: Vec<(u32, Image)>,
    /// The mark, its uniforms, and each mark this frame: its record and its
    /// palette's group, the palette as it is or the copy as it was.
    mark: Built,
    mark_uniforms: DrawUniforms,
    marks: Vec<([u32; DRAW_WORDS], u32)>,
    /// The last frame's draws; each palette's copy as last drawn, by its
    /// handle (handle, buffer, group, bytes); copies let go, released a
    /// frame later, once no frame's commands still read them.
    was: Vec<Drawn>,
    was_palettes: Vec<(u32, u32, u32, u32)>,
    let_go: Vec<(u32, u32)>,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        SkinnedPass {
            lit: pipelines.build_world(g, PIPELINE_SKINNED),
            depth: pipelines.build_gated(
                g,
                PIPELINE_SKINNED_DEPTH,
                Format::depth32float,
                Stencil::Gated,
            ),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            skins: Vec::new(),
            mark: pipelines.build_gated(
                g,
                PIPELINE_SKINNED_MARK,
                Format::depth32float,
                Stencil::Unused,
            ),
            mark_uniforms: DrawUniforms::new(DRAW_BYTES, align),
            marks: Vec::new(),
            was: Vec::new(),
            was_palettes: Vec::new(),
            let_go: Vec::new(),
        }
    }

    /// The marks this frame: each draw whose record, uniform or palette
    /// (`touched`, the handles this frame's lists upload or rewrite) is not
    /// what the last frame drew, where it is and where it was; each draw the
    /// last frame drew and this one does not, where it was. Whether any.
    pub fn prepare_marks<'a>(
        &mut self,
        g: &Gpu,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
        touched: &[u32],
        uploads: &mut Uploads,
    ) -> bool {
        let now: Vec<Drawn> = draws.map(|d| (*d, place_of(uniforms, d))).collect();
        let mut marks: Vec<(Drawn, u32)> = Vec::new();
        let was_group = |handle: u32| {
            self.was_palettes
                .iter()
                .find(|p| p.0 == handle)
                .map(|p| p.2)
        };
        for &(d, u) in &now {
            let was = self.was.iter().find(|w| w.0[1] == d[1]);
            if was.is_some_and(|&(wd, wu)| wd == d && wu == u) && !touched.contains(&d[1]) {
                continue;
            }
            if let Some(palette) = uploads.group(g, d[1]) {
                marks.push(((d, u), palette));
            }
            if let Some(&w) = was {
                if let Some(palette) = was_group(w.0[1]) {
                    marks.push((w, palette));
                }
            }
        }
        for &w in &self.was {
            if !now.iter().any(|(d, _)| d[1] == w.0[1]) {
                if let Some(palette) = was_group(w.0[1]) {
                    marks.push((w, palette));
                }
            }
        }
        self.mark_uniforms.write(
            g,
            self.mark.layouts[1],
            marks.len() as u32,
            marks.iter(),
            true,
            |buf, ((_, u), _)| {
                buf[..24].copy_from_slice(bytes(u));
                buf[24..DRAW_BYTES as usize].fill(0);
            },
        );
        self.marks = marks.into_iter().map(|((d, _), p)| (d, p)).collect();
        !self.marks.is_empty()
    }

    /// The handles of the palettes the last frame drew, which name its draws
    /// as the kernel keys them (pxl-kernel region.rs `drawn`).
    pub fn drawn_keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.was.iter().map(|w| w.0[1])
    }

    /// Group 0's layout of the mark (tile_mark.wgsl).
    pub fn mark_layout(&self) -> u32 {
        self.mark.layouts[0]
    }

    /// The marks `prepare_marks` made drawn into `pass`, `frame` their group
    /// 0: the eye's frame and the screen tiles' words (tile_mark.wgsl).
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frame: u32, uploads: &Uploads) {
        let Some(slots) = self.mark_uniforms.slots() else {
            return;
        };
        for (k, &(d, palette)) in self.marks.iter().enumerate() {
            let Some((mesh, vertex_cnt, idx_cnt)) = uploads.get(d[0]) else {
                continue;
            };
            let Some(skin) = self.image_of(d[2]).and_then(|i| skin_made(i.0)) else {
                continue;
            };
            pass.set_pipeline(self.mark.pipeline);
            pass.set_bind_group(0, frame, &[]);
            let (group, off) = slots.at(k as u32);
            pass.set_bind_group(1, group, &[off]);
            pass.set_bind_group(2, skin, &[]);
            pass.set_bind_group(3, palette, &[]);
            draw_model(pass, &self.lit, mesh, vertex_cnt, idx_cnt, 1);
        }
    }

    /// What this frame drew kept for the next's marks where it was: the
    /// draws and their uniforms, and each palette copied in `encoder`,
    /// after the frame's draws read it.
    pub fn keep<'a>(
        &mut self,
        g: &Gpu,
        encoder: u32,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
        uploads: &Uploads,
    ) {
        for (buf, group) in self.let_go.drain(..) {
            release(group);
            release(buf);
        }
        let now: Vec<Drawn> = draws.map(|d| (*d, place_of(uniforms, d))).collect();
        let mut kept = Vec::with_capacity(now.len());
        for (d, _) in &now {
            let Some((src, size)) = uploads.buf_of(d[1]) else {
                continue;
            };
            let p = match self
                .was_palettes
                .iter()
                .position(|p| p.0 == d[1] && p.3 == size)
            {
                Some(i) => self.was_palettes.swap_remove(i),
                None => {
                    let buf = g.buf(size, buffer_usage::STORAGE | buffer_usage::COPY_DST);
                    let group = g.bind_group(self.lit.layouts[3], &[buf_entry(0, buf)]);
                    (d[1], buf, group, size)
                }
            };
            g.copy_buf(encoder, (src, 0), (p.1, 0), size);
            kept.push(p);
        }
        for p in self.was_palettes.drain(..) {
            self.let_go.push((p.1, p.2));
        }
        self.was_palettes = kept;
        self.was = now;
    }

    pub fn set_skins(&mut self, skins: &[(u32, Image)]) {
        self.skins.clear();
        self.skins.extend_from_slice(skins);
    }

    fn image_of(&self, model: u32) -> Option<Image> {
        self.skins
            .iter()
            .find(|&&(m, _)| m == model)
            .map(|&(_, image)| image)
    }

    /// Every palette and skin group the draws will bind is made here, so
    /// drawing makes nothing that outlives its frame; a skin whose image
    /// lands later is drawn on a later frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone,
        uniforms: &[f32],
        moved: bool,
        uploads: &mut Uploads,
    ) {
        self.uniforms.write(
            g,
            self.lit.layouts[1],
            draws.len() as u32,
            draws.clone(),
            moved,
            |buf, d| {
                let u = uniform_of(uniforms, d);
                // Translation, turn about Z, opacity, scale, padding (skinned.wgsl `Draw`).
                buf[..24].copy_from_slice(bytes(&u[..6]));
                buf[24..DRAW_BYTES as usize].fill(0);
            },
        );
        for d in draws {
            uploads.group(g, d[1]);
            if let Some(image) = self.image_of(d[2]) {
                skin_group(g, self.lit.layouts[2], image);
            }
        }
    }

    /// Lit, or their depth from the sun (`Stage::Depth`), `frame` the group
    /// 0 the stage reads.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
        stage: Stage,
        frame: u32,
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        let pipeline = match stage {
            Stage::Depth => self.depth.pipeline,
            _ => self.lit.pipeline,
        };
        draw_models(pass, &self.lit, pipeline, frame, slots, draws, |d| {
            let (mesh, vertex_cnt, idx_cnt) = uploads.get(d[0])?;
            let skin = skin_made(self.image_of(d[2])?.0)?;
            Some(ModelDraw {
                groups: [skin, uploads.group_of(d[1])?],
                mesh: Some((mesh, vertex_cnt, idx_cnt)),
                instances: 1,
            })
        });
    }

    pub fn release(mut self) {
        self.uniforms.release();
        self.mark_uniforms.release();
        for (_, buf, group, _) in self.was_palettes.drain(..) {
            release(group);
            release(buf);
        }
        for (buf, group) in self.let_go.drain(..) {
            release(group);
            release(buf);
        }
    }
}

/// A draw's place, its uniform's first words: translation, turn about Z,
/// opacity, scale, padding (skinned.wgsl `Draw`).
fn place_of(uniforms: &[f32], d: &[u32; DRAW_WORDS]) -> [f32; 6] {
    let u = uniform_of(uniforms, d);
    [u[0], u[1], u[2], u[3], u[4], u[5]]
}
