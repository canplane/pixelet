// pxl-render/gpu/pass/mark.rs

//! The marks of the listed draws that changed. A frame that is not drawn
//! whole draws again only its claimed tiles (gpu/tile.rs), and a draw that
//! changed claims its tiles by its mark. The mark sets the words of the
//! screen's tiles it covers, and of the light map's tiles where the draw
//! casts a shadow (tile_mark.wgsl). The draw is drawn twice for its mark,
//! with its vertices laid as its depth pass lays them: where it was, by the
//! frame words the last frame drew it with, and where it is, by this
//! frame's. The marked tiles then hold the pixels it covered then and covers
//! now, each at its own frame's jitter.
//!
//! A draw counts as changed in three cases. The turn changed it (gpu/draw.rs
//! `Change`), and it is drawn as it was and as it is. The upload it reads
//! moved; `touched` holds the handles this frame's lists upload, rewrite or
//! free. Or it drifts with the wind and the wind's drift moved; each of its
//! two draws then takes its own frame's drift. To draw a draw where it was,
//! `CopyCache` keeps each upload the draws read as the last frame drew
//! it. It copies an upload after the frame has read it, whenever the lists
//! moved it since.

use crate::frame::shader::record::UPLOAD_LAYOUT;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, Format, Gpu, Layout, Stencil, TrackedRenderPass,
};
use crate::gpu::draw::{drifted, drifts, Change, Draw, DrawTable, Position, DRAW_WORDS};
use crate::gpu::pass::model::{UniformBuffer, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::upload::UploadTable;

/// Gives the handle of the upload a draw reads, 0 for none.
pub(crate) type UploadOf = fn(&[u32; DRAW_WORDS]) -> u32;

/// Writes a mark's uniform into its slot as the mark's pipeline reads it.
type PutMark = fn(&mut [u8], &Draw);

/// Says whether the pass draws a draw at all. A draw it does not draw marks
/// nothing.
pub(crate) type Shown = fn(&[u32; DRAW_WORDS]) -> bool;

/// What a frame's marks are made from. `draws` are the draws as the turn
/// left them, and `changes` are the turn's changes. `touched` holds the
/// handles the frame's lists touch, sorted. `drift` holds how far the wind
/// has carried the clouds, first as this frame draws them and then as the
/// last frame drew them (word.rs `frame::DRIFT_Q`).
pub struct DamageInput<'a> {
    pub draws: DrawTable<'a>,
    pub changes: &'a [Change],
    pub touched: &'a [u32],
    pub drift: [Position; 2],
}

/// Writes a mark's uniform as the draw's own, whole, as its lit pass reads
/// it.
pub(crate) fn put_whole(buf: &mut [u8], (_, u): &Draw) {
    buf[..DRAW_BYTES as usize].copy_from_slice(bytes(u));
}

/// The draw as a frame draws it at the wind's drift `drift`. The drift is
/// taken into its uniform (gpu/draw.rs `drifted`), so the mark's shader adds
/// no frame's drift to it again.
fn drawn_at((d, mut u): Draw, drift: Position) -> Draw {
    drifted(&mut u, drift);
    (d, u)
}

/// Hands `take` each draw of `pipeline` that a frame marks
/// (`MarkPass::prepare`), as it is drawn at its frame's drift, with whether it
/// is the draw as it is (true) or as it was. A draw the turn changed is
/// handed as it is, as it was, or both, by which of the two it has. A draw
/// whose upload moved, or that drifts while the drift moved, is handed both
/// ways. `read` names the upload a draw reads, and only a draw that `shown`
/// passes is marked.
pub(crate) fn marked(
    pipeline: u32,
    (read, shown): (UploadOf, Shown),
    m: &DamageInput,
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
    let drift_changed = m.drift[0] != m.drift[1];
    if m.touched.is_empty() && !drift_changed {
        return;
    }
    let changed = |d: &[u32; DRAW_WORDS]| read(d) != 0 && m.touched.binary_search(&read(d)).is_ok();
    for d in m.draws.of(pipeline) {
        if !shown(d) || m.draws.changed(d[3]) {
            continue;
        }
        let Some(d) = m.draws.now(d[3]) else { continue };
        if changed(&d.0) || (drift_changed && drifts(&d.1)) {
            take(drawn_at(d, m.drift[0]), true);
            take(drawn_at(d, m.drift[1]), false);
        }
    }
}

/// The marks of one pipeline's draws (`MarkPass::new`). `marks` holds this
/// frame's marks, each a draw's record with the group of the upload it reads
/// (0 where it reads none), as it is or as it was. The marks as they are come
/// first, `now_cnt` of them.
pub struct MarkPass {
    pipeline: Pipeline,
    marks_of: u32,
    read: UploadOf,
    put: PutMark,
    shown: Shown,
    uniforms: UniformBuffer,
    marks: Vec<([u32; DRAW_WORDS], u32)>,
    now_cnt: usize,
}

impl MarkPass {
    /// The marks of the draws of pipeline `marks_of`, drawn by pipeline `id`
    /// into a target whose depth is attached read only. `read` names the
    /// upload each draw reads, and `put` writes each mark's uniform. Only a
    /// draw that `shown` passes is marked.
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
        (id, marks_of): (u32, u32),
        align: u32,
        (read, put, shown): (UploadOf, PutMark, Shown),
    ) -> MarkPass {
        MarkPass {
            pipeline: pipelines.build_masked(g, id, Format::depth32float, Stencil::Unused),
            marks_of,
            read,
            put,
            shown,
            uniforms: UniformBuffer::new(DRAW_BYTES, align),
            marks: Vec::new(),
            now_cnt: 0,
        }
    }

    /// The layout of the marks' group 0 (tile_mark.wgsl). It binds the frame
    /// words, the tiles' words, their uniform and the box of the tiles the
    /// frame's marks cover.
    pub fn layout(&self) -> Layout {
        self.pipeline.layouts[0]
    }

    /// Makes this frame's marks of the pipeline's draws (`marked`) and says
    /// whether there are any. A draw marked as it is reads its upload as it
    /// is now. A draw marked as it was reads its upload as the last frame drew
    /// it (`last_frame_uploads`). A mark whose upload has no group on the
    /// device is left out.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        m: &DamageInput,
        uploads: &mut UploadTable,
        last_frame_uploads: &CopyCache,
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

    /// Draws the marks `prepare` made into `pass`. `frames` holds two group 0s
    /// (tile_mark.wgsl): the first over this frame's words, for the marks as
    /// they are, and the second over the last frame's, for the marks as they
    /// were. The marks' uniforms are group 1. `bind` is given each mark's
    /// record and its upload's group. It binds the groups past 1 and the mesh
    /// and draws the mark, or skips it when what it draws with is not on the
    /// device.
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

    /// Pushes onto `out` the uploads `draws` read. These are the uploads
    /// `CopyCache` keeps a copy of.
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

/// A copy of an upload that a marking draw reads, as the last frame drew
/// it.
#[derive(Clone, Copy)]
struct Copy {
    handle: u32,
    buf: u32,
    group: u32,
    size: u32,
    /// Whether it holds the upload as the last frame drew it.
    valid: bool,
}

/// A copy of each upload the marking draws read, as the last frame drew it,
/// found by the upload's handle. Each copy's group is in the one layout that
/// every upload's group has (record.rs `UPLOAD_LAYOUT`). A copy that no draw
/// of this frame reads is let go, and it is released before the next frame's
/// commands.
#[derive(Default)]
pub struct CopyCache {
    copies: Vec<Copy>,
    let_go: Vec<(u32, u32)>,
    layout: Option<Layout>,
}

impl CopyCache {
    /// Releases the copies let go before, then makes a copy for each upload
    /// `handles` names that has no copy of its size. A copy of another size
    /// is let go. The caller calls it before the frame's commands, so a frame
    /// that is dropped loses no copy.
    pub fn prepare(&mut self, g: &Gpu, handles: &[u32], uploads: &UploadTable) {
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
                valid: false,
            });
        }
    }

    /// Copies in `encoder` each upload `handles` names whose copy does not
    /// hold it as drawn: the lists moved it this frame (`touched`), or the
    /// copy is new or forgotten. The caller encodes it after the frame's
    /// draws have read the uploads. A copy that no handle names is let go.
    pub fn keep(
        &mut self,
        g: &Gpu,
        encoder: u32,
        handles: &[u32],
        touched: &[u32],
        uploads: &UploadTable,
    ) {
        let mut k = 0;
        while k < self.copies.len() {
            let c = &mut self.copies[k];
            if handles.binary_search(&c.handle).is_err() {
                let c = self.copies.swap_remove(k);
                self.let_go.push((c.buf, c.group));
                continue;
            }
            if !c.valid || touched.binary_search(&c.handle).is_ok() {
                let src = uploads.buf_of(c.handle).filter(|&(_, size)| size == c.size);
                if let Some((src, size)) = src {
                    g.copy_buf(encoder, (src, 0), (c.buf, 0), size);
                }
                c.valid = src.is_some();
            }
            k += 1;
        }
    }

    /// The group of the copy of upload `handle` as the last frame drew it, or
    /// none where no copy holds it.
    pub fn group_of(&self, handle: u32) -> Option<u32> {
        self.copies
            .iter()
            .find(|c| c.handle == handle && c.valid)
            .map(|c| c.group)
    }

    /// Marks every copy as not holding what the last frame drew, as after a
    /// frame is dropped or put off. The next frame drawn copies each again.
    pub fn forget(&mut self) {
        for c in &mut self.copies {
            c.valid = false;
        }
    }
}
