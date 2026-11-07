// pxl-render/gpu/pass/mark.rs

//! What a listed draw that changed marks: the screen's tiles and, where it
//! casts, the light's map's (tile_mark.wgsl), by itself drawn where it was,
//! by the frame's words the last frame drew it with, and where it is, by
//! this frame's, its vertices as its depth pass lays them, each fragment
//! setting its tile's word: the pixels it covered then and covers now,
//! each frame at its own jitter. A draw changed where the turn changed it
//! (gpu/draw.rs `Change`), as it was and as it is; or where the upload it
//! reads moved (`touched`, the handles this frame's lists upload, rewrite or
//! free); or, one that drifts with the wind, where the wind's drift moved,
//! each drawn at its own frame's drift. What the last frame drew is kept for
//! that: each upload the draws read, copied after the frame read it where the
//! lists moved it since (`UploadCopies`).

use crate::frame::shader::record::UPLOAD_LAYOUT;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, Format, Gpu, Layout, Stencil, TrackedRenderPass,
};
use crate::gpu::draw::{drifted, drifts, Change, Draw, Draws, Position, DRAW_WORDS};
use crate::gpu::pass::model::{DrawUniforms, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::upload::Uploads;

/// The upload a draw reads, 0 for none.
pub(crate) type UploadOf = fn(&[u32; DRAW_WORDS]) -> u32;

/// A mark's uniform written as its pipeline reads it.
type PutMark = fn(&mut [u8], &Draw);

/// Whether a draw of the pass is drawn at all.
pub(crate) type Shown = fn(&[u32; DRAW_WORDS]) -> bool;

/// What a frame's marks are made from: the draws as the turn left them, its
/// changes, the handles its lists touch, sorted, and how far the wind has
/// carried the clouds as this frame draws them and as the last frame drew
/// them (word.rs `frame::DRIFT_Q`).
pub struct MarkInput<'a> {
    pub draws: Draws<'a>,
    pub changes: &'a [Change],
    pub touched: &'a [u32],
    pub drift: [Position; 2],
}

/// A mark's uniform: the draw's, as its lit pass reads it.
pub(crate) fn put_whole(buf: &mut [u8], (_, u): &Draw) {
    buf[..DRAW_BYTES as usize].copy_from_slice(bytes(u));
}

/// A draw as a frame draws it, at the wind's drift `drift` (gpu/draw.rs
/// `drifted`), so its mark reads no frame's.
fn drawn_at((d, mut u): Draw, drift: Position) -> Draw {
    drifted(&mut u, drift);
    (d, u)
}

/// Each draw of `pipeline` a frame marks (`Marks::prepare`) handed to
/// `take`, as it is drawn, at its frame's drift, with whether it is as it is
/// (true) or as it was: each the turn changed, the one or both it has; and
/// each whose upload moved, or that drifts where the drift moved, both. A
/// draw reads the upload `read` names, and only one `shown` marks.
pub(crate) fn marked(
    pipeline: u32,
    (read, shown): (UploadOf, Shown),
    m: &MarkInput,
    mut take: impl FnMut(Draw, bool),
) {
    let mine = |d: &Draw| d.0[4] == pipeline && shown(&d.0);
    for c in m.changes {
        if let Some(d) = m.draws.now(c.slot).filter(mine) {
            take(drawn_at(d, m.drift[0]), true);
        }
        if let Some(w) = c.was.filter(mine) {
            take(drawn_at(w, m.drift[1]), false);
        }
    }
    let drift_moved = m.drift[0] != m.drift[1];
    if m.touched.is_empty() && !drift_moved {
        return;
    }
    let moved = |d: &[u32; DRAW_WORDS]| read(d) != 0 && m.touched.binary_search(&read(d)).is_ok();
    for d in m.draws.of(pipeline) {
        if !shown(d) || m.draws.changed(d[3]) {
            continue;
        }
        let Some(d) = m.draws.now(d[3]) else { continue };
        if moved(&d.0) || (drift_moved && drifts(&d.1)) {
            take(drawn_at(d, m.drift[0]), true);
            take(drawn_at(d, m.drift[1]), false);
        }
    }
}

/// One pipeline's marks: the pipeline they are drawn by and the draws' it
/// marks (`marks`), the upload a draw reads (`read`, 0 for none), a mark's
/// uniform as the pipeline reads it (`put`) and whether a draw is drawn
/// (`shown`), the marks' uniforms, and this frame's marks, each
/// its record and the group of the upload it reads, as it is or as it was (0
/// where it reads none), those as they are first, `now_cnt` of them.
pub struct Marks {
    pipeline: Pipeline,
    marks_of: u32,
    read: UploadOf,
    put: PutMark,
    shown: Shown,
    uniforms: DrawUniforms,
    marks: Vec<([u32; DRAW_WORDS], u32)>,
    now_cnt: usize,
}

impl Marks {
    /// The marks of pipeline `marks_of`'s draws, drawn by pipeline `id` into
    /// a target whose depth is attached read only, each draw reading the
    /// upload `read` names, each mark's uniform written by `put`, a draw
    /// `shown` alone marking.
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        (id, marks_of): (u32, u32),
        align: u32,
        (read, put, shown): (UploadOf, PutMark, Shown),
    ) -> Marks {
        Marks {
            pipeline: pipelines.build_gated(g, id, Format::depth32float, Stencil::Unused),
            marks_of,
            read,
            put,
            shown,
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            marks: Vec::new(),
            now_cnt: 0,
        }
    }

    /// Group 0's layout (tile_mark.wgsl): the frame, the tiles' words, their
    /// uniform and the box of the tiles the frame's marks cover.
    pub fn layout(&self) -> Layout {
        self.pipeline.layouts[0]
    }

    /// The marks this frame of the pipeline's draws: each the turn changed,
    /// where it is, its upload as it is, and where it was, its upload as the
    /// last frame drew it (`last_frame_uploads`); and each whose upload
    /// moved, or that drifts where the drift moved, both. Whether any.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        m: &MarkInput,
        uploads: &mut Uploads,
        last_frame_uploads: &UploadCopies,
    ) -> bool {
        let (read, put, shown) = (self.read, self.put, self.shown);
        let mut now: Vec<(Draw, u32)> = Vec::new();
        let mut was: Vec<(Draw, u32)> = Vec::new();
        marked(self.marks_of, (read, shown), m, |d, is| {
            let group = match read(&d.0) {
                0 => Some(0),
                h if is => uploads.group(g, h),
                h => last_frame_uploads.group_of(h),
            };
            let list = if is { &mut now } else { &mut was };
            list.extend(group.map(|group| (d, group)));
        });
        let now_cnt = now.len();
        now.append(&mut was);
        self.uniforms.write(
            g,
            self.pipeline.layouts[1],
            now.len() as u32,
            now.iter(),
            true,
            |buf, (d, _)| put(buf, d),
        );
        self.marks.clear();
        self.marks
            .extend(now.into_iter().map(|((d, _), group)| (d, group)));
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
            let group = g.bind_group(layout, &[buf_entry(0, buf), buf_entry(1, buf)]);
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
            if handles.binary_search(&c.handle).is_err() {
                let c = self.copies.swap_remove(k);
                self.let_go.push((c.buf, c.group));
                continue;
            }
            if !c.held || touched.binary_search(&c.handle).is_ok() {
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
