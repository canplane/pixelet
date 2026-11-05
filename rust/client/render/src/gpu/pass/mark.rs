// pxl-render/gpu/pass/mark.rs

//! What a listed draw that changed marks: the screen's tiles and, where it
//! casts, the light's map's (tile_mark.wgsl), by itself drawn where it was,
//! by the frame's words the last frame drew it with, and where it is, by
//! this frame's, its vertices as its depth pass lays them, each fragment
//! setting its tile's word: the pixels it covered then and covers now,
//! each frame at its own jitter. A draw changed where no draw the last frame
//! drew draws as it does, by its record, its uniform's place in the list
//! aside, and its uniform; or where the upload it reads moved (`touched`, the
//! handles this frame's lists upload, rewrite or free). A draw the last frame
//! drew that none of this frame's draws as marks where it was. What the last
//! frame drew is kept for that: each pass's draws (`Marks::keep`), and each
//! upload they read, copied after the frame read it where the lists moved it
//! since (`UploadCopies`).

use crate::frame::draw::{uniform_of, DRAW_WORDS};
use crate::frame::shader::record::UPLOAD_LAYOUT;
use crate::frame::shader::word::DRAW_FLOATS;
use crate::gpu::api::{
    buf_entry, buffer_usage, release, Format, Gpu, Layout, Stencil, TrackedRenderPass,
};
use crate::gpu::pass::model::{DrawUniforms, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::upload::Uploads;

/// A draw as a frame drew it: its record and its uniform.
pub type Draw = ([u32; DRAW_WORDS], [f32; DRAW_FLOATS]);

/// The upload a draw reads, 0 for none.
type UploadOf = fn(&[u32; DRAW_WORDS]) -> u32;

/// A mark's uniform written as its pipeline reads it.
type PutMark = fn(&mut [u8], &Draw);

/// Whether two draws draw alike: their records but where their uniforms lie
/// in the list, and their uniforms, bit for bit.
fn alike(a: &Draw, b: &Draw) -> bool {
    let record = |d: &[u32; DRAW_WORDS]| [d[0], d[1], d[2], d[4]];
    record(&a.0) == record(&b.0) && a.1.map(f32::to_bits) == b.1.map(f32::to_bits)
}

fn drawn_of(uniforms: &[f32], d: &[u32; DRAW_WORDS]) -> Draw {
    let mut u = [0.0; DRAW_FLOATS];
    u.copy_from_slice(uniform_of(uniforms, d));
    (*d, u)
}

/// One pass's marks: its pipeline, the upload a draw reads (`read`, 0 for
/// none) and a mark's uniform as the pipeline reads it (`put`), the marks'
/// uniforms, this frame's marks, each its record and the group of the upload
/// it reads, as it is or as it was (0 where it reads none), those as they
/// are first, `now_cnt` of them, and what the last frame drew.
pub struct Marks {
    pipeline: Pipeline,
    read: UploadOf,
    put: PutMark,
    uniforms: DrawUniforms,
    marks: Vec<([u32; DRAW_WORDS], u32)>,
    now_cnt: usize,
    was: Vec<Draw>,
}

impl Marks {
    /// Marks drawn by pipeline `id` into a target whose depth is attached
    /// read only, each draw reading the upload `read` names, each mark's
    /// uniform written by `put`.
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        id: u32,
        align: u32,
        (read, put): (UploadOf, PutMark),
    ) -> Marks {
        Marks {
            pipeline: pipelines.build_gated(g, id, Format::depth32float, Stencil::Unused),
            read,
            put,
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            marks: Vec::new(),
            now_cnt: 0,
            was: Vec::new(),
        }
    }

    /// Group 0's layout (tile_mark.wgsl): the frame, the tiles' words, their
    /// uniform and the box of the tiles the frame's marks cover.
    pub fn layout(&self) -> Layout {
        self.pipeline.layouts[0]
    }

    /// The marks this frame of `draws`: each that no draw the last frame
    /// drew draws as, where it is, its upload as it is; each the last frame
    /// drew that none of this frame's draws as, where it was, its upload as
    /// the last frame drew it (`last_frame_uploads`); one whose upload moved,
    /// both. Whether any.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
        touched: &[u32],
        uploads: &mut Uploads,
        last_frame_uploads: &UploadCopies,
    ) -> bool {
        let (read, put) = (self.read, self.put);
        let now: Vec<Draw> = draws.map(|d| drawn_of(uniforms, d)).collect();
        let moved = |d: &Draw| read(&d.0) != 0 && touched.contains(&read(&d.0));
        let mut marks: Vec<(Draw, u32)> = Vec::new();
        for d in &now {
            if !moved(d) && self.was.iter().any(|w| alike(w, d)) {
                continue;
            }
            let group = match read(&d.0) {
                0 => Some(0),
                h => uploads.group(g, h),
            };
            marks.extend(group.map(|group| (*d, group)));
        }
        let now_cnt = marks.len();
        for w in &self.was {
            if !moved(w) && now.iter().any(|d| alike(w, d)) {
                continue;
            }
            let group = match read(&w.0) {
                0 => Some(0),
                h => last_frame_uploads.group_of(h),
            };
            marks.extend(group.map(|group| (*w, group)));
        }
        self.uniforms.write(
            g,
            self.pipeline.layouts[1],
            marks.len() as u32,
            marks.iter(),
            true,
            |buf, (d, _)| put(buf, d),
        );
        self.marks = marks
            .into_iter()
            .map(|((d, _), group)| (d, group))
            .collect();
        self.now_cnt = now_cnt;
        !self.marks.is_empty()
    }

    /// The marks `prepare` made drawn into `pass`, `frames` their group 0
    /// (tile_mark.wgsl) over the frame's words of a draw as it is and of one
    /// as it was, their uniforms group 1; `bind` binds each one's groups past
    /// those and its mesh, given its upload's group, and draws it, or passes
    /// it by where what it draws with is not on the device.
    pub fn draw(
        &self,
        pass: &mut TrackedRenderPass,
        frames: [u32; 2],
        mut bind: impl FnMut(&mut TrackedRenderPass, &[u32; DRAW_WORDS], u32),
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        for (k, (d, upload)) in self.marks.iter().enumerate() {
            pass.set_pipeline(self.pipeline.id);
            pass.set_bind_group(0, frames[usize::from(k >= self.now_cnt)], &[]);
            let (group, off) = slots.at(k as u32);
            pass.set_bind_group(1, group, &[off]);
            bind(pass, d, *upload);
        }
    }

    /// The uploads `draws` read, into `out`: what is kept of them
    /// (`UploadCopies`).
    pub fn reads<'a>(
        &self,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        out: &mut Vec<u32>,
    ) {
        out.extend(draws.map(self.read).filter(|&h| h != 0));
    }

    /// What this frame drew of `draws`, kept for the next's marks where it
    /// was; this frame's marks done with.
    pub fn keep<'a>(
        &mut self,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
    ) {
        self.was.clear();
        self.was.extend(draws.map(|d| drawn_of(uniforms, d)));
        self.marks.clear();
        self.now_cnt = 0;
    }

    pub fn release(&mut self) {
        self.uniforms.release();
    }
}

/// An upload a draw that marks reads, copied as the last frame drew it.
#[derive(Clone, Copy)]
struct Copy {
    handle: u32,
    buf: u32,
    group: u32,
    size: u32,
    /// Whether it holds the upload as the last frame drew it.
    held: bool,
}

/// Each upload the draws that mark read, copied as the last frame drew it,
/// by its handle, its group in the one layout every upload's is
/// (record.rs `UPLOAD_LAYOUT`); copies let go, released before the next
/// frame's commands, once none of this frame's reads them.
#[derive(Default)]
pub struct UploadCopies {
    copies: Vec<Copy>,
    let_go: Vec<(u32, u32)>,
    layout: Option<Layout>,
}

impl UploadCopies {
    /// Before the frame's commands, so a frame dropped loses none: a copy
    /// made for each upload `handles` names that has none of its size, one
    /// of another size let go; and those let go before released.
    pub fn prepare(&mut self, g: &Gpu, handles: &[u32], uploads: &Uploads) {
        for (buf, group) in self.let_go.drain(..) {
            release(group);
            release(buf);
        }
        for &h in handles {
            let Some((_, size)) = uploads.buf_of(h) else {
                continue;
            };
            match self.copies.iter().position(|c| c.handle == h) {
                Some(i) if self.copies[i].size == size => continue,
                Some(i) => {
                    let c = self.copies.swap_remove(i);
                    self.let_go.push((c.buf, c.group));
                }
                None => {}
            }
            let layout = *self.layout.get_or_insert_with(|| g.layout(&UPLOAD_LAYOUT));
            let buf = g.buf(size, buffer_usage::STORAGE | buffer_usage::COPY_DST);
            let group = g.bind_group(layout, &[buf_entry(0, buf)]);
            self.copies.push(Copy {
                handle: h,
                buf,
                group,
                size,
                held: false,
            });
        }
    }

    /// Each upload `handles` names copied in `encoder`, after the frame's
    /// draws read it, where its copy does not hold it as drawn: the lists
    /// moved it this frame (`touched`), or the copy is new or forgotten.
    /// Copies no handle names let go.
    pub fn keep(
        &mut self,
        g: &Gpu,
        encoder: u32,
        handles: &[u32],
        touched: &[u32],
        uploads: &Uploads,
    ) {
        let mut k = 0;
        while k < self.copies.len() {
            let c = &mut self.copies[k];
            if !handles.contains(&c.handle) {
                let c = self.copies.swap_remove(k);
                self.let_go.push((c.buf, c.group));
                continue;
            }
            if !c.held || touched.contains(&c.handle) {
                let src = uploads.buf_of(c.handle).filter(|&(_, size)| size == c.size);
                if let Some((src, size)) = src {
                    g.copy_buf(encoder, (src, 0), (c.buf, 0), size);
                }
                c.held = src.is_some();
            }
            k += 1;
        }
    }

    /// The group of the copy of upload `handle` as the last frame drew it;
    /// none where none holds it.
    pub fn group_of(&self, handle: u32) -> Option<u32> {
        self.copies
            .iter()
            .find(|c| c.handle == handle && c.held)
            .map(|c| c.group)
    }

    /// The copies no longer hold what the last frame drew, a frame dropped
    /// or put off since: each is copied again by the next frame drawn.
    pub fn forget(&mut self) {
        for c in &mut self.copies {
            c.held = false;
        }
    }
}
