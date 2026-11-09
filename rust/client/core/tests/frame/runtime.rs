//! The runtime's end of the core's rings, kept the way @xpute/runtime's doorbell and the
//! client's services keep it (xpute/ts/runtime/src/ipc/doorbell.ts,
//! ts/client/src/service/core/). It lays the front before the first turn, sends the UI's
//! commands and waits for its calls' answers. Everything a turn posts is read before the next
//! turn is rung: the file calls are answered (fs.rs), the stop report is kept, and the
//! snapshot is read where the core names it.

use std::collections::BTreeMap;

use pxl_core::sys::machine::MACHINE;
use xpute_core::ipc::frame::{ACKREQ, PROGRAM_MAJOR, RES};
use xpute_core::ipc::head;
use xpute_core::ipc::ring::Ring;
use xpute_core::mem::Memory;
use xpute_kit::status::errno::Errno;
use xpute_runtime::native::{ring, turns, Core};

use crate::fs::Fs;
use crate::spec::{self, Protocol, V};

/// What the last snapshot said: the share of the renderer's pipelines made so far, and
/// whether a zoom is still easing in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shown {
    pub renderer_progress: f64,
    pub zooming: bool,
}

pub struct Runtime {
    mem: Memory,
    sq: Ring,
    cq: Ring,
    next_call: u32,
    replies: BTreeMap<u32, i32>,
    pub ui: Protocol,
    xpute: Protocol,
    fs: Protocol,
    disk: Fs,
    /// The core's stop report, as SYS_ABORT carried it.
    pub abort: Option<String>,
    pub shown: Shown,
    /// Each command the core refused, by name, with its errno.
    pub refused: Vec<String>,
}

fn major_of(cmd: u32) -> u32 {
    cmd >> 8
}

impl Runtime {
    /// Lays the front on `core`'s memory where its machine places it, before the first turn.
    pub fn lay(core: Core) -> Runtime {
        let m = MACHINE;
        let (base, bytes) = core.mem();
        // SAFETY: the core's memory, reserved for the process's life; its front is the
        // runtime's to write before the first turn, and its rings each side's by their rules.
        let mem = unsafe { Memory::new(base.as_ptr().expose_provenance(), bytes) };
        // SAFETY: the front's header and rings, which no turn has read yet.
        let (sq, cq) = unsafe {
            head::write(mem, m.head_at() as usize, &m.head());
            (
                Ring::init(
                    mem,
                    m.sq_at() as usize,
                    m.sq_cap,
                    m.sq_slots_at(),
                    m.slot_bytes,
                ),
                Ring::init(
                    mem,
                    m.cq_at() as usize,
                    m.cq_cap,
                    m.cq_slots_at(),
                    m.slot_bytes,
                ),
            )
        };
        let fs = Protocol::load("xpute/spec/abi/fs.json");
        Runtime {
            mem,
            sq,
            cq,
            next_call: 0,
            replies: BTreeMap::new(),
            ui: Protocol::load("spec/client/core/door/ui.json"),
            xpute: Protocol::load_each(&["xpute/spec/abi/sys.json", "xpute/spec/abi/gpu.json"]),
            disk: Fs::new(),
            fs,
            abort: None,
            shown: Shown::default(),
            refused: Vec::new(),
        }
    }

    /// Pushes a frame on the submission ring with the packet `write` writes, or with no
    /// packet when it writes none. When the ring is full, a turn is rung to make room.
    fn push(
        &mut self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        write: impl Fn(&mut [u8]) -> Option<u32>,
    ) {
        let mut buf = vec![0u8; self.sq.slot_bytes as usize];
        let pkt = write(&mut buf).map(|n| &buf[..n as usize]);
        loop {
            match self.sq.push(tag, cmd, flags, result, pkt) {
                Errno::OK => return,
                Errno::EAGAIN => {
                    self.turn(0.0);
                }
                e => panic!("command {cmd:#x} refused by the ring: {e:?}"),
            }
        }
    }

    /// Sends the command `name`, the UI's or xpute's (abi/sys.json, abi/gpu.json), with its
    /// fields given by name. The core takes it on the next turn.
    pub fn send(&mut self, name: &str, vals: &[(&str, V)]) {
        self.submit(name, vals, 0, 0);
    }

    fn submit(&mut self, name: &str, vals: &[(&str, V)], tag: u32, flags: u32) {
        let command = self
            .ui
            .get(name)
            .or_else(|| self.xpute.get(name))
            .unwrap_or_else(|| panic!("no command {name}"))
            .clone();
        self.push(tag, command.val, flags, 0, |slot| {
            spec::write(&command, vals, slot)
        });
    }

    /// Makes the call `name`, the UI's or xpute's, ringing turns with a deadline of 0 until
    /// the core answers, and returns the result.
    pub fn call(&mut self, name: &str, vals: &[(&str, V)]) -> i32 {
        self.next_call = self.next_call.wrapping_add(1).max(1);
        let tag = self.next_call;
        self.submit(name, vals, tag, ACKREQ);
        loop {
            if let Some(r) = self.replies.remove(&tag) {
                return r;
            }
            assert!(
                self.turn(0.0).is_some(),
                "the core stopped before it answered {name}"
            );
        }
    }

    /// Rings a turn with `deadline_ms` and reads its posts after it. Returns when the core
    /// wants the next turn, or None when the core has stopped.
    pub fn turn(&mut self, deadline_ms: f64) -> Option<f64> {
        let wake = ring(deadline_ms);
        // The wakes the turns asked for are taken here, so the next ring reports only its
        // own.
        let _ = turns();
        self.drain();
        wake
    }

    /// Reads the completion ring until it is empty, then submits the file calls'
    /// completions.
    fn drain(&mut self) {
        let mut results = Vec::new();
        while let Some(f) = self.cq.peek() {
            let pkt = self.cq.pkt(&f).expect("the core's frame names its slot");
            if f.flags & RES != 0 {
                if f.result < 0 && f.tag == 0 {
                    self.refused
                        .push(format!("{} {:?}", self.name(f.cmd), Errno::of(f.result)));
                }
                self.replies.insert(f.tag, f.result);
            } else if major_of(f.cmd) >= PROGRAM_MAJOR {
                self.program(f.cmd, pkt);
            } else if major_of(f.cmd) == self.fs.member("Major", "FS") {
                let cmd = self.fs.name_of(f.cmd).expect("an fs command").to_string();
                let read = spec::read(pkt.unwrap_or(&[]));
                let field =
                    |name: &str| spec::num(read.at(self.fs.at(&cmd, name)).expect(name)) as u32;
                match cmd.as_str() {
                    "FS_OPENAT" => {
                        let path = read
                            .at(self.fs.at(&cmd, "path"))
                            .and_then(|c| c.get_str_ref())
                            .expect("an open's path");
                        let res = self
                            .disk
                            .open(field("fd"), field("root"), field("flags"), path);
                        results.push((field("fd"), f.cmd, res));
                    }
                    "FS_READ" => {
                        let bytes = self.disk.read(field("fd"));
                        let n = bytes.len().min(field("len") as usize);
                        // SAFETY: the bytes the read names, the runtime's in the gap between turns.
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                bytes.as_ptr(),
                                self.mem.at::<u8>(field("at") as usize),
                                n,
                            );
                        }
                    }
                    "FS_WRITE" => self.disk.write(field("fd")),
                    "FS_CLOSE" => results.push((field("fd"), f.cmd, self.disk.close(field("fd")))),
                    _ => panic!("the core posted {cmd}"),
                }
            } else if f.cmd == self.xpute.number("SYS_ABORT") {
                self.abort = Some(self.report(pkt.expect("a report's packet")));
            } else {
                panic!("the core posted {:#x}, which no one here takes", f.cmd);
            }
            self.cq.advance();
        }
        let openat = self.fs.command("FS_OPENAT");
        let reply = spec::Command {
            val: openat.val,
            fields: openat.reply.clone(),
            reply: Vec::new(),
        };
        for (fd, cmd, res) in results {
            if cmd == reply.val {
                self.push(fd, cmd, RES, res, |slot| {
                    spec::write(&reply, &[("detail", V::Num(0.0))], slot)
                });
            } else {
                self.push(fd, cmd, RES, res, |_| None);
            }
        }
    }

    fn name(&self, cmd: u32) -> String {
        self.ui
            .name_of(cmd)
            .or_else(|| self.xpute.name_of(cmd))
            .map_or_else(|| format!("{cmd:#x}"), str::to_string)
    }

    /// Takes what the core posts to the UI. The snapshot is read where it lies, and every
    /// other post is ignored.
    fn program(&mut self, cmd: u32, pkt: Option<&[u8]>) {
        if cmd != self.ui.number("UI_SNAPSHOT") {
            return;
        }
        let read = spec::read(pkt.expect("a snapshot's event"));
        let field =
            |name: &str| spec::num(read.at(self.ui.at("UI_SNAPSHOT", name)).expect(name)) as usize;
        // SAFETY: the snapshot the event names, the runtime's until its head passes the event.
        let bytes =
            unsafe { core::slice::from_raw_parts(self.mem.at::<u8>(field("at")), field("len")) };
        let s = spec::read(bytes);
        self.shown = Shown {
            renderer_progress: spec::num(
                s.at(self.ui.record_at("Snapshot", "renderer_progress"))
                    .expect("the progress"),
            ),
            zooming: spec::flag(
                s.at(self.ui.record_at("Snapshot", "zooming"))
                    .expect("the zoom"),
            ),
        };
    }

    fn report(&self, pkt: &[u8]) -> String {
        let r = spec::read(pkt);
        let at = |name: &str| r.at(self.xpute.at("SYS_ABORT", name)).expect(name);
        let num = |name: &str| spec::num(at(name)) as i64;
        let word = |low: &str, high: &str| (num(high) as u64) << 32 | num(low) as u64;
        let errnum = num("errnum") as i32;
        format!(
            "{} at {}:{}:{} ({}, {})",
            Errno::of(errnum).map_or_else(|| errnum.to_string(), |e| format!("{e:?}")),
            at("file").get_str_ref().unwrap_or("?"),
            num("line"),
            num("column"),
            word("a_low", "a_high"),
            word("b_low", "b_high"),
        )
    }
}
