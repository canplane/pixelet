// pxl-render/gpu/track.rs

//! What a pass's calls are held to as they are encoded: an indirect call never reads its count
//! from a buffer its usage scope binds writable, and a render pass that declares the pipelines
//! it draws sets no other.
//!
//! What writes args is a buffer made `INDIRECT` bound writable in a group set in the pass. A
//! compute dispatch is one usage scope, so the groups set for it are checked; a whole render
//! pass is one usage scope, so every group set in it is checked.

use crate::frame::shader::floor::BIND_GROUPS as GROUPS;
use pxl_gpu_guest::encode::Id;

/// A pass being recorded: its handle, whether it computes, and the groups set in it now, by
/// index.
struct Open {
    pass: Id,
    compute: bool,
    groups: [Id; GROUPS],
    /// A render pass's: every group set in it, and every buffer an indirect draw of it read,
    /// as a render pass is one usage scope whole.
    drawn_groups: Vec<Id>,
    drawn_args: Vec<Id>,
    /// A render pass's declared pipelines, by their records' numbers (frame/shader/access.rs
    /// `Step::in_pass`); none where the pass declares none.
    declared: &'static [u32],
}

/// The args buffers the device holds, each with the source line that made it
/// (gpu/api.rs `Gpu::buf`), the args each bind group binds writable, each at its
/// binding, the passes being recorded, and the record each render pipeline was made from.
#[derive(Default)]
pub struct Tracker {
    args: Vec<(Id, u32)>,
    writes: Vec<(Id, Vec<(u32, Id)>)>,
    open: Vec<Open>,
    recs: Vec<(Id, u32)>,
}

impl Tracker {
    /// Records a buffer made at source line `line`. It is an args buffer when it was made
    /// `INDIRECT`.
    pub fn note_buffer(&mut self, buf: Id, indirect: bool, line: u32) {
        self.let_go(buf);
        if indirect {
            self.args.push((buf, line));
        }
    }

    /// Whether `buf` is an args buffer.
    pub fn is_args(&self, buf: Id) -> bool {
        self.args.iter().any(|&(b, _)| b == buf)
    }

    /// The source line that made args buffer `buf`, 0 for another.
    pub fn made_at(&self, buf: Id) -> u32 {
        self.args
            .iter()
            .find(|&&(b, _)| b == buf)
            .map_or(0, |&(_, line)| line)
    }

    /// Records a bind group made with `writable` bound as writable storage, each at its
    /// binding. The args buffers among them are what a call through the group writes.
    pub fn grouped(&mut self, group: Id, writable: impl Iterator<Item = (u32, Id)>) {
        self.let_go(group);
        let args: Vec<(u32, Id)> = writable.filter(|(_, b)| self.is_args(*b)).collect();
        if !args.is_empty() {
            self.writes.push((group, args));
        }
    }

    /// Records that an object was let go. Its handle may name another object next.
    pub fn let_go(&mut self, id: Id) {
        self.args.retain(|&(b, _)| b != id);
        self.writes.retain(|(g, _)| *g != id);
        self.recs.retain(|&(p, _)| p != id);
    }

    /// Records a render pipeline made from record `rec` (frame/shader/record.rs
    /// `pipeline`).
    pub fn recorded(&mut self, pipeline: Id, rec: u32) {
        self.let_go(pipeline);
        self.recs.push((pipeline, rec));
    }

    /// Records that render pass `pass` declares it draws `pipelines` alone, by their records'
    /// numbers.
    pub fn declared(&mut self, pass: Id, pipelines: &'static [u32]) {
        if let Some(o) = self.open(pass) {
            o.declared = pipelines;
        }
    }

    /// When `pipeline` is set in a pass that declares what it draws and is not among those,
    /// its record's number (`u32::MAX` for a pipeline made from no record).
    pub fn undeclared(&self, pass: Id, pipeline: Id) -> Option<u32> {
        let o = self.open_of(pass)?;
        if o.declared.is_empty() {
            return None;
        }
        let rec = self
            .recs
            .iter()
            .find(|&&(p, _)| p == pipeline)
            .map_or(u32::MAX, |&(_, r)| r);
        (!o.declared.contains(&rec)).then_some(rec)
    }

    /// The binding at which `group` binds `buf` writable, if it does.
    fn writes_at(&self, group: Id, buf: Id) -> Option<u32> {
        let (_, args) = self.writes.iter().find(|(w, _)| *w == group)?;
        args.iter()
            .find(|(_, b)| *b == buf)
            .map(|&(binding, _)| binding)
    }

    /// Where a group of `pass` binds `buf` writable, for an indirect call of the pass that
    /// reads its count from `buf`: the group, as its index among those set or, for a render
    /// pass, as its handle, and the binding. WebGPU refuses a usage scope that holds `buf` both
    /// as `INDIRECT` and as writable storage. `buf` is then noted as read in a render pass, so
    /// a group set later in it is held to it too (`binds_read`).
    pub fn binds_writable(&mut self, pass: Id, buf: Id) -> Option<(u32, u32)> {
        let o = self.open_of(pass)?;
        if !o.compute {
            let found = o
                .drawn_groups
                .iter()
                .find_map(|&g| self.writes_at(g, buf).map(|b| (g, b)));
            if let Some(o) = self.open(pass)
                && !o.drawn_args.contains(&buf)
            {
                o.drawn_args.push(buf);
            }
            return found;
        }
        o.groups
            .iter()
            .enumerate()
            .find_map(|(idx, g)| self.writes_at(*g, buf).map(|binding| (idx as u32, binding)))
    }

    /// Where `group`, set in render pass `pass`, binds writable a buffer from which an indirect
    /// draw of the pass read its count: that buffer and the binding.
    pub fn binds_read(&self, pass: Id, group: Id) -> Option<(Id, u32)> {
        let o = self.open_of(pass).filter(|o| !o.compute)?;
        o.drawn_args
            .iter()
            .find_map(|&buf| self.writes_at(group, buf).map(|b| (buf, b)))
    }

    /// Records a pass begun as `pass`.
    pub fn opened(&mut self, pass: Id, compute: bool) {
        self.open.push(Open {
            pass,
            compute,
            groups: [0; GROUPS],
            drawn_groups: Vec::new(),
            drawn_args: Vec::new(),
            declared: &[],
        });
    }

    fn open_of(&self, pass: Id) -> Option<&Open> {
        self.open.iter().find(|o| o.pass == pass)
    }

    fn open(&mut self, pass: Id) -> Option<&mut Open> {
        self.open.iter_mut().find(|o| o.pass == pass)
    }

    /// Records that `group` was bound at `idx` of `pass`.
    pub fn bound(&mut self, pass: Id, idx: u32, group: Id) {
        if let Some(o) = self.open(pass) {
            o.groups[idx as usize] = group;
            if !o.compute && !o.drawn_groups.contains(&group) {
                o.drawn_groups.push(group);
            }
        }
    }

    /// Records that `pass` ended.
    pub fn closed(&mut self, pass: Id) {
        self.open.retain(|o| o.pass != pass);
    }
}

#[cfg(test)]
#[path = "track.test.rs"]
mod test;
