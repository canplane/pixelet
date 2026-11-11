//! The UI's end of the core, kept the way the device's runtime keeps it (xpute-runtime native,
//! xpute/ts/runtime/src/start.native.ts): the front's header is laid and the runtime attached
//! before the first turn; the UI's commands are posted, written by their protocols' runtime
//! ends, turns are run, and what the core sent is taken after each. The file device is xpute-runtime's own, and this harness answers only
//! with a file's bytes (fs.rs): an open's, and a written file's commit, which it drops. The
//! stop report is kept, and the snapshot is read from the memory its event named.

use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use pxl_core::door::cmd_runtime::{self as ui, Major, Snapshot};
use pxl_core::sys::manifest::MANIFEST;
use xpute_core::io::frame::{cmd, cmd_major, cmd_minor, ACKREQ, RES};
use xpute_core::io::head;
use xpute_core::mem::Memory;
use xpute_gpu::runtime::abi as gpu;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::PacketWriter;
use xpute_runtime::abi::fs::{Command as FsCommand, FsOpen};
use xpute_runtime::abi::{sys, Export, Outcome};
use xpute_runtime::abort_report;
use xpute_runtime::native::batch::{self, ALIGN, COMMIT, HDR_WORDS, STOPPED};
use xpute_runtime::native::{self, run, Core};

use crate::fs;

/// A GPU command, under the major the program gives the GPU (ui.json `Major::GPU`).
pub struct Gpu<E>(pub E);

impl<E: Export> Export for Gpu<E> {
    const NUMBER: u32 = cmd(Major::GPU as u32, E::NUMBER);
    const FIELDS: u32 = E::FIELDS;
    fn write(&self, w: &mut PacketWriter) {
        self.0.write(w);
    }
}

/// What the last snapshot said: the share of the renderer's pipelines made so far, and
/// whether a zoom is still easing in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shown {
    pub renderer_progress: f64,
    pub zooming: bool,
}

/// Whether the runtime has told the UI that frames wait for it (`native::notify_on`), which
/// the runtime's own thread sets.
static TOLD: Mutex<bool> = Mutex::new(false);
static TOLD_CV: Condvar = Condvar::new();

extern "C" fn told() {
    *TOLD.lock().unwrap_or_else(PoisonError::into_inner) = true;
    TOLD_CV.notify_all();
}

/// How long the harness waits for the runtime to hand over the stop report of a core that
/// stopped.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// A frame the core sent, as `take` hands it over.
struct Down<'a> {
    tag: u32,
    cmd: u32,
    flags: u32,
    result: i32,
    pkt: &'a [u8],
    named: &'a [u8],
}

/// The frames of `down`, a batch (xpute spec/io/batch.json), in the order sent.
fn frames(down: &[u8]) -> Vec<Down<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    let align = ALIGN as usize;
    while at < down.len() {
        let word = |k: u32| {
            let k = at + 4 * k as usize;
            u32::from_le_bytes(down[k..k + 4].try_into().expect("a word"))
        };
        let (pkt_len, named_len) = (word(batch::PKT) as usize, word(batch::NAMED) as usize);
        let pkt_at = at + 4 * HDR_WORDS as usize;
        let named_at = (pkt_at + pkt_len).next_multiple_of(align);
        out.push(Down {
            tag: word(batch::TAG),
            cmd: word(batch::CMD),
            flags: word(batch::FLAGS),
            result: word(batch::RESULT) as i32,
            pkt: &down[pkt_at..pkt_at + pkt_len],
            named: &down[named_at..named_at + named_len],
        });
        at = (named_at + named_len).next_multiple_of(align);
    }
    out
}

pub struct Runtime {
    /// The tag of the last request; 0 is a send's.
    next_tag: u32,
    replies: BTreeMap<u32, i32>,
    /// The core's stop report, as SYS_ABORT carried it.
    pub abort: Option<String>,
    pub shown: Shown,
    /// Each command the core refused, by name, with its errno.
    pub refused: Vec<String>,
}

impl Runtime {
    /// Lays the front's header on `core`'s memory where its manifest places it, and attaches
    /// the runtime with the file device, before the first turn. The core's calls are refused,
    /// as no program answers them here.
    pub fn lay(core: Core) -> Runtime {
        let m = MANIFEST;
        let (base, bytes) = core.mem();
        // SAFETY: the core's memory, reserved for the process's life; its front is the
        // runtime's to write before the first turn.
        let mem = unsafe { Memory::new(base.as_ptr().expose_provenance(), bytes) };
        // SAFETY: the front's header, which no turn has read yet.
        unsafe { head::write(mem, m.head_at() as usize, &m.head()) };
        native::answer_with(None);
        native::notify_on(Some(told));
        // SAFETY: the core's memory, whose front is laid.
        unsafe { native::attach(base.as_ptr().expose_provenance(), bytes, true) };
        Runtime {
            next_tag: 0,
            replies: BTreeMap::new(),
            abort: None,
            shown: Shown::default(),
            refused: Vec::new(),
        }
    }

    /// Posts `e` for the core under `tag` with `flags`.
    fn submit<E: Export>(&mut self, e: &E, tag: u32, flags: u32) {
        let mut buf = vec![0u8; MANIFEST.pkt_bytes as usize];
        let mut w = PacketWriter::new(&mut buf, E::FIELDS);
        e.write(&mut w);
        let n = w.finish().expect("the packet fits a frame");
        native::post(tag, E::NUMBER, flags, 0, &buf[..n as usize]);
    }

    /// Sends `e`, the UI's command or xpute's. The core takes it on the next turn.
    pub fn send<E: Export>(&mut self, e: &E) {
        self.submit(e, 0, 0);
    }

    /// Requests `e` with ACKREQ under a tag of its own, and runs turns with a deadline of 0
    /// until its reply comes with a later turn's batch; returns the reply's result.
    pub fn request<E: Export>(&mut self, e: &E) -> i32 {
        self.next_tag = self.next_tag.wrapping_add(1).max(1);
        let tag = self.next_tag;
        self.submit(e, tag, ACKREQ);
        loop {
            if let Some(r) = self.replies.remove(&tag) {
                return r;
            }
            assert!(
                self.turn(0.0).is_some(),
                "the core stopped before it answered {}",
                name(E::NUMBER)
            );
        }
    }

    /// Runs a turn with `deadline_ms`, and takes what the runtime has told of since. Returns
    /// when the core wants the next turn, or None when the core has stopped, once the runtime
    /// has handed over all it sent.
    pub fn turn(&mut self, deadline_ms: f64) -> Option<f64> {
        let wake = run(deadline_ms);
        if wake.is_some() {
            self.drain();
            return wake;
        }
        let mut stopped = false;
        while !stopped {
            let mut g = TOLD.lock().unwrap_or_else(PoisonError::into_inner);
            while !*g {
                let (next, waited) = TOLD_CV
                    .wait_timeout(g, STOP_WAIT)
                    .unwrap_or_else(PoisonError::into_inner);
                assert!(!waited.timed_out(), "the stop was never handed over");
                g = next;
            }
            drop(g);
            stopped = self.drain();
        }
        None
    }

    /// Takes what the runtime has told of, as a device's UI does when told: a reply to a
    /// command, what the core tells the UI, its stop report, and what the file device asks of
    /// the program, an open to read, answered with the file's bytes, and a written file's
    /// commit, which is dropped. Answers whether the batch says the core has stopped.
    fn drain(&mut self) -> bool {
        if !core::mem::take(&mut *TOLD.lock().unwrap_or_else(PoisonError::into_inner)) {
            return false;
        }
        native::answer();
        let batch = native::take();
        let mut stopped = false;
        for f in frames(&batch) {
            if f.cmd == STOPPED {
                stopped = true;
                continue;
            }
            if f.flags & RES != 0 {
                if f.result < 0 && f.tag == 0 {
                    self.refused
                        .push(format!("{} {:?}", name(f.cmd), Errno::of(f.result)));
                }
                self.replies.insert(f.tag, f.result);
                continue;
            }
            let pkt = (!f.pkt.is_empty()).then_some(f.pkt);
            let major = cmd_major(f.cmd);
            if f.cmd == COMMIT {
                // What is written is dropped, as the run reads only the files it ships.
                native::post(f.tag, FsCommand::FS_CLOSE as u32, RES, 0, &[]);
            } else if f.cmd == FsCommand::FS_OPEN as u32 {
                let open = FsOpen::read(f.pkt).expect("an open's packet");
                let tokens = open.path.iter().collect::<Vec<_>>();
                match fs::bytes(&tokens) {
                    Ok(bytes) => native::fs_complete_open(f.tag, bytes.len() as i32, 0, bytes),
                    Err(e) => native::fs_complete_open(f.tag, e as i32, 0, Vec::new()),
                }
            } else if f.cmd == FsCommand::FS_CLOSE as u32 {
                // An open the core closed before it completed: every open here is answered
                // at once, so there is nothing to abort.
            } else if ui::MAJORS.contains(&major) {
                // What the UI alone takes is answered by nothing, as on the device.
                ui::serve(self, f.tag, f.cmd, &pkt, f.named).expect("a UI's command");
            } else if sys::MAJORS.contains(&major) {
                // A command is completed where it fails or asks for an answer, as the
                // device's UI completes it (@xpute/runtime start.native.ts).
                match sys::serve(self, f.tag, f.cmd, &pkt, f.named) {
                    Ok(Some(r)) if f.flags & ACKREQ != 0 || r < 0 => {
                        native::post(f.tag, f.cmd, RES, r, &[]);
                    }
                    Ok(_) => {}
                    Err(e) => native::post(f.tag, f.cmd, RES, e.errnum as i32, &[]),
                }
            } else {
                panic!("the core sent {:#x}, which no one here takes", f.cmd);
            }
        }
        stopped
    }
}

/// What the core sends the UI: the snapshot, read from the memory its event names, and
/// nothing else, as no sensor runs here.
impl ui::Imports for Runtime {
    fn sensor_start(&mut self, _: ui::SensorStart) -> Outcome {
        Ok(None)
    }

    fn sensor_stop(&mut self, _: ui::SensorStop) -> Outcome {
        Ok(None)
    }

    fn ui_snapshot(&mut self, _: ui::UiSnapshot, named: &[u8]) -> Outcome {
        let s = Snapshot::read(named).expect("a snapshot");
        self.shown = Shown {
            renderer_progress: s.renderer_progress().expect("the progress"),
            zooming: s.zooming().expect("the zoom"),
        };
        Ok(None)
    }

    fn ui_view(&mut self, _: ui::UiView) -> Outcome {
        Ok(None)
    }
}

/// The core's stop report, kept; its pace is the runtime's thread's (xpute-runtime
/// native/serve.rs), and never comes here.
impl sys::Imports for Runtime {
    fn sys_abort(&mut self, e: sys::SysAbort<'_>) -> Outcome {
        self.abort = Some(abort_report(&e));
        Ok(None)
    }

    fn sys_pace(&mut self, _: sys::SysPace) -> Outcome {
        Ok(None)
    }
}

/// A command's name: the UI's, xpute's own, or the GPU's under its major.
pub fn name(cmd: u32) -> String {
    let gpu = (cmd_major(cmd) == Major::GPU as u32)
        .then(|| gpu::Command::of(cmd_minor(cmd)).map(|c| format!("{c:?}")))
        .flatten();
    ui::Command::of(cmd)
        .map(|c| format!("{c:?}"))
        .or_else(|| sys::Command::of(cmd).map(|c| format!("{c:?}")))
        .or(gpu)
        .unwrap_or_else(|| format!("{cmd:#x}"))
}
