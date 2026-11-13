// pxl-render/gpu/split.rs

//! Where an encoder's work is cut into command buffers: no indirect dispatch
//! or draw reads args written earlier in the command buffer it is recorded
//! in. WebKit's driver on Metal hangs the GPU at far zoom when an indirect
//! call reads args written before it in the same command buffer, and not
//! when they were written in a command buffer submitted before it.
//!
//! What writes args is found as it is recorded: a buffer made `INDIRECT`,
//! bound writable in a group of the pass when it dispatches or draws, or
//! copied into. What reads them is each indirect call. A compute pass whose
//! indirect call would read args written in its command buffer is ended
//! there and begun again in a new one, its pipeline and groups set again and
//! the new part one more pass of its span; a render pass, whose indirect
//! draws are known only as it draws, begins a new command buffer wherever
//! args were written in the one it would join. A new pass keeps the rule
//! whatever it writes and reads.

use crate::frame::shader::floor::BIND_GROUPS as GROUPS;
use crate::gpu::span_spec::GpuSpanName;
use xpute_gpu_guest::encode::Id;

/// An encoder as its recorder names it, and the command buffers it is cut
/// into: the encoder recorded into now, those ended before it in the order
/// they run, and the args buffers written in the one recorded into now.
struct Chain {
    named: Id,
    at: Id,
    ended: Vec<Id>,
    written: Vec<Id>,
}

/// A pass being recorded: the handle its recorder names it by, the pass it is recorded as now,
/// and its encoder as named. A compute pass also holds what its next part is begun with where
/// it is cut: its pipeline, its groups and their offsets, and the span it is timed in.
pub struct Open {
    named: Id,
    pub at: Id,
    encoder: Id,
    compute: bool,
    pub pipeline: Id,
    pub groups: [(Id, Vec<u32>); GROUPS],
    pub span: Option<GpuSpanName>,
    /// A render pass's: every group set in it, and every buffer an indirect draw of it read,
    /// as a render pass is one usage scope whole.
    drawn_groups: Vec<Id>,
    drawn_args: Vec<Id>,
    /// A render pass's declared pipelines, by their records' numbers (frame/shader/sequence.rs
    /// `Step::in_pass`); none where the pass declares none.
    declared: &'static [u32],
}

/// The args buffers the device holds, each with the source line that made it
/// (gpu/api.rs `Gpu::buf`), the args each bind group binds writable, each at its
/// binding, the encoders and passes being recorded, and the record each render
/// pipeline was made from.
#[derive(Default)]
pub struct Split {
    args: Vec<(Id, u32)>,
    writes: Vec<(Id, Vec<(u32, Id)>)>,
    chains: Vec<Chain>,
    open: Vec<Open>,
    recs: Vec<(Id, u32)>,
}

impl Split {
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

    /// Records a render pipeline made from record `record` (frame/shader/record.rs
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

    /// Records that `encoder` was begun. It records into itself until it is cut.
    pub fn begun(&mut self, encoder: Id) {
        self.chains.push(Chain {
            named: encoder,
            at: encoder,
            ended: Vec::new(),
            written: Vec::new(),
        });
    }

    fn chain(&self, encoder: Id) -> Option<&Chain> {
        self.chains.iter().find(|c| c.named == encoder)
    }

    fn chain_mut(&mut self, encoder: Id) -> Option<&mut Chain> {
        self.chains.iter_mut().find(|c| c.named == encoder)
    }

    /// The encoder `encoder` is recorded into now: itself, where it was
    /// never cut or is not one of these.
    pub fn at(&self, encoder: Id) -> Id {
        self.chain(encoder).map_or(encoder, |c| c.at)
    }

    /// Whether a render pass on `encoder` begins a command buffer of its own, which it does
    /// when any args were written in the one it would join.
    pub fn draws_apart(&self, encoder: Id) -> bool {
        self.chain(encoder).is_some_and(|c| !c.written.is_empty())
    }

    /// Whether an indirect call of `pass` that reads `buf` is recorded in a command buffer of
    /// its own, which it is when `buf` was written in the one the pass records into now. This
    /// is the one place a frame is cut.
    pub fn cuts(&self, pass: Id, buf: Id) -> bool {
        self.open_of(pass)
            .and_then(|o| self.chain(o.encoder))
            .is_some_and(|c| c.written.contains(&buf))
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
    /// as `INDIRECT` and as writable storage. A compute dispatch is one usage scope, so the
    /// groups set for it are checked. A whole render pass is one usage scope, so every group
    /// set in it is checked. `buf` is then noted as read, so a group set later in a render pass
    /// is held to it too (`binds_read`).
    pub fn binds_writable(&mut self, pass: Id, buf: Id) -> Option<(u32, u32)> {
        let o = self.open_of(pass)?;
        if !o.compute {
            let found = o
                .drawn_groups
                .iter()
                .find_map(|&g| self.writes_at(g, buf).map(|b| (g, b)));
            if let Some(o) = self.open(pass) {
                if !o.drawn_args.contains(&buf) {
                    o.drawn_args.push(buf);
                }
            }
            return found;
        }
        o.groups
            .iter()
            .enumerate()
            .find_map(|(idx, (g, _))| self.writes_at(*g, buf).map(|binding| (idx as u32, binding)))
    }

    /// Where `group`, set in render pass `pass`, binds writable a buffer from which an indirect
    /// draw of the pass read its count: that buffer and the binding.
    pub fn binds_read(&self, pass: Id, group: Id) -> Option<(Id, u32)> {
        let o = self.open_of(pass).filter(|o| !o.compute)?;
        o.drawn_args
            .iter()
            .find_map(|&buf| self.writes_at(group, buf).map(|b| (buf, b)))
    }

    /// Records that `encoder` records into `next` from here, and that the command buffer it
    /// recorded into before is ended.
    pub fn split(&mut self, encoder: Id, next: Id) {
        if let Some(c) = self.chain_mut(encoder) {
            c.ended.push(core::mem::replace(&mut c.at, next));
            c.written.clear();
        }
    }

    /// Records a pass begun on `encoder` as `pass`, timed in `span`.
    pub fn opened(&mut self, pass: Id, encoder: Id, compute: bool, span: Option<GpuSpanName>) {
        self.open.push(Open {
            named: pass,
            at: pass,
            encoder,
            compute,
            pipeline: 0,
            groups: Default::default(),
            span,
            drawn_groups: Vec::new(),
            drawn_args: Vec::new(),
            declared: &[],
        });
    }

    fn open_of(&self, pass: Id) -> Option<&Open> {
        self.open.iter().find(|o| o.named == pass)
    }

    /// The open pass `pass`: what it is recorded as now, its encoder as named, and whether it
    /// is a compute pass.
    pub fn open(&mut self, pass: Id) -> Option<&mut Open> {
        self.open.iter_mut().find(|o| o.named == pass)
    }

    /// The pass `pass` is recorded as now, which is itself when it was never cut.
    pub fn pass_at(&self, pass: Id) -> Id {
        self.open_of(pass).map_or(pass, |o| o.at)
    }

    /// The encoder `pass` was begun on, as named, and whether it computes.
    pub fn pass_on(&self, pass: Id) -> Option<(Id, bool)> {
        self.open_of(pass).map(|o| (o.encoder, o.compute))
    }

    /// Records that `pipeline` was set on `pass`.
    pub fn piped(&mut self, pass: Id, pipeline: Id) {
        if let Some(o) = self.open(pass) {
            o.pipeline = pipeline;
        }
    }

    /// Records that `group` was bound at `idx` of `pass` with `offs`.
    pub fn bound(&mut self, pass: Id, idx: u32, group: Id, offs: &[u32]) {
        if let Some(o) = self.open(pass) {
            o.groups[idx as usize] = (group, offs.to_vec());
            if !o.compute && !o.drawn_groups.contains(&group) {
                o.drawn_groups.push(group);
            }
        }
    }

    /// Records that a call of `pass` ran, so the args buffers its groups bind writable are
    /// written.
    pub fn ran(&mut self, pass: Id) {
        let Some(o) = self.open_of(pass) else {
            return;
        };
        let encoder = o.encoder;
        let written: Vec<Id> = o
            .groups
            .iter()
            .filter_map(|(g, _)| self.writes.iter().find(|(w, _)| w == g))
            .flat_map(|(_, args)| args.iter().map(|&(_, b)| b))
            .collect();
        if let Some(c) = self.chain_mut(encoder) {
            for a in written {
                if !c.written.contains(&a) {
                    c.written.push(a);
                }
            }
        }
    }

    /// Records a copy into `to` on `encoder`, which writes `to` when it is an args buffer.
    pub fn copied(&mut self, encoder: Id, to: Id) {
        if self.is_args(to) {
            if let Some(c) = self.chain_mut(encoder) {
                if !c.written.contains(&to) {
                    c.written.push(to);
                }
            }
        }
    }

    /// Records that `pass` ended.
    pub fn closed(&mut self, pass: Id) {
        self.open.retain(|o| o.named != pass);
    }

    /// The command buffers `encoder` was cut into, in the order they run, which are forgotten
    /// here. It is `encoder` alone when it was never cut.
    pub fn submitted(&mut self, encoder: Id) -> Vec<Id> {
        match self.chains.iter().position(|c| c.named == encoder) {
            Some(k) => {
                let c = self.chains.swap_remove(k);
                let mut all = c.ended;
                all.push(c.at);
                all
            }
            None => vec![encoder],
        }
    }
}

#[cfg(test)]
#[path = "split.test.rs"]
mod test;
