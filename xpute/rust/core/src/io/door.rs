// xpute-core/io/door.rs

//! The door is how the core takes the frames its runtime sends it, and sends
//! its own (`crate::host`). A turn receives every frame that waits, one at a
//! time into the door's receive frame, and applies each in order. The core
//! sends its replies and its own commands as it goes, each laid in the door's
//! send frame, which the runtime takes before `send` returns. Both frames lie
//! in the core's memory wherever the core put them, and each call names its
//! frame by its offset.
//!
//! Each command is routed by its major to its protocol's generated `apply`:
//! xpute's own (abi/sys.json, abi/fs.json) to the door's end of them, which
//! answers NOP and hands the program the rest as what each means, and every
//! other to the program, as its own (`Event::Program`). The program is handed
//! each completion of a request it made too (`Event::Completion`), which its
//! protocol's generated `completion` reads.
//!
//! The core's imports, the commands it sends the runtime, are frames too, each
//! sent by its protocol's generated function. A send goes without ACKREQ and
//! is answered only where it fails; a request goes with ACKREQ, and the
//! runtime completes it once, on a later turn, with a frame of its tag flagged
//! RES. A call, which is answered before it returns, is laid in the send frame
//! and handed to the runtime's `call`. The runtime routes each import by its
//! command's major.
//!
//! - R1. Memory an import names (`Import::named`), or a reply (`Reply`), is
//!   taken with the frame, so it is the core's again once `send` returns: a
//!   file's write is, and a reply's bytes. A file's
//!   read names memory the runtime fills later: it is the runtime's until the
//!   FS_CLOSE of its descriptor is completed, and the file table holds the
//!   buffer until then (io/file.rs). Memory a call names is the runtime's
//!   only until the call returns.
//! - R2. A completion finds its request by its tag, never by when it came.
//! - R3. A call never nests: the runtime answers it without calling into the
//!   core, and the core makes no call while another is unanswered.

use core::panic::Location;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, HDR_SZ, WORD_SZ};

use core::cell::Cell;

use crate::abi::fs::{self, FsChanged};
use crate::abi::handle::tag_of;
use crate::abi::sys::{self, Major, Nop, SysAbort, SysFrame, SysTurn};
use crate::global::Global;
use crate::host::{self, answer_of};
use crate::io::frame::{self, Frame, PROGRAM_MAJOR, WORD_BYTES};
use crate::io::head;
use crate::manifest::Manifest;
use crate::mem::{off_of_slice, Memory};

/// An import is what xpute-gen generates from a protocol's table (`spec/cmd`):
/// its command number, the number of children its packet is written as,
/// whether it goes with ACKREQ, its writer, and the memory it names, where it
/// is and how long (0 long for none).
pub trait Import {
    const NUMBER: u32;
    const FIELDS: u32;
    const ACKREQ: bool;
    fn write(&self, w: &mut PacketWriter);
    fn named(&self) -> (u32, u32);
}

/// How many bytes from the end of a stop report's file path are kept when
/// the whole path would not fit in a frame.
const ABORT_PATH_BYTES: usize = 32;

/// The bytes of a stop report with its file path left out, which every
/// frame's packet must hold so that a stop is always reported (`abort`): XTP's
/// header, the root's table of the report's children, its numbers of four
/// bytes each, and an empty string, its length word and terminator padded to
/// a word (xpute-kit wire/xtp/encode.rs).
pub const ABORT_PKT_BYTES: u32 = {
    let numbers = HDR_SZ + WORD_SZ * (1 + SysAbort::FIELDS) + 4 * (SysAbort::FIELDS - 1);
    (numbers.next_multiple_of(WORD_SZ) + WORD_SZ + 1).next_multiple_of(WORD_SZ)
};

/// Writes a frame's packet into the room it is given, and answers its length,
/// or none when it does not fit.
type Write<'a> = &'a dyn Fn(&mut [u8]) -> Option<u32>;

/// What the program answers an event: a reply, or none for a command it does
/// not know, which is replied to with ENOSYS.
pub type Outcome = Result<Option<Reply>, MarshalError>;

/// A reply to a command: its value, and the core's memory it names, which goes
/// with the reply's frame as with any frame's (R1), empty for none. What it
/// names lies in the core's memory, as the heap does; a native build's statics
/// do not, and naming one is a bug (EFAULT).
#[derive(Clone, Copy, Debug)]
pub struct Reply {
    pub val: i32,
    pub named: &'static [u8],
}

impl From<i32> for Reply {
    fn from(val: i32) -> Reply {
        Reply { val, named: &[] }
    }
}

/// The door hands the program a change in its life that its runtime tells it
/// of, a turn the scheduling policy grants, a file changed under it, a command
/// of the program's own, with its number and its packet, for its protocol's
/// generated `apply`, or the completion of a request it made, for its
/// protocol's generated `completion`. Nothing answers a completion, so what
/// the program returns for one is dropped. The drain runs before any phase's budget, so the program should
/// record what it is handed and return. xpute may add kinds: a program matches
/// the kinds it takes and leaves the rest to a wildcard.
#[non_exhaustive]
pub enum Event<'a> {
    Life(Life),
    /// The scheduling policy grants a turn (SYS_TURN), which carries the wall
    /// clock.
    Turn(SysTurn),
    /// Frame pacing's grant for the next turn (SYS_FRAME).
    Frame(SysFrame),
    Completion(Completion<'a>),
    /// A file changed under the core, so what the core read of it is stale
    /// (FS_CHANGED).
    Changed(FsChanged<'a>),
    Program(u32, Option<&'a [u8]>),
}

/// The runtime's completion of a request the core made, as the frame that
/// completes it carries it, for the request's protocol to read (its generated
/// `completion`): the command, the handle the request went under, the result,
/// and the reply's packet.
#[derive(Clone, Copy, Debug)]
pub struct Completion<'a> {
    pub cmd: u32,
    pub tag: u32,
    pub res: i32,
    pub pkt: Option<&'a [u8]>,
}

/// The core's life is as its runtime tells it (abi/sys.json). What each event
/// means the program keeps or resets is the program's to decide; a program
/// with nothing to do answers `Ok(None)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// A runtime has attached, for the first time or again (SYS_ATTACH).
    Attach,
    /// The core is about to be frozen; this is its last sure moment to save
    /// (SYS_SUSPEND).
    Suspend,
    /// The core has been woken (SYS_RESUME).
    Resume,
}

pub struct Door {
    mem: Memory,
    receive_at: u32,
    send_at: u32,
    /// A frame's room: its words and the most its packet takes.
    frame_bytes: u32,
    /// The generation of the last handle with no value issued (`tag`).
    tags: Cell<u16>,
}

impl Door {
    /// A door over the receive frame at `receive_at` and the send frame at
    /// `send_at`, each `frame_bytes` long and 8-aligned, which nothing else
    /// holds for the door's life.
    pub fn new(mem: Memory, receive_at: u32, send_at: u32, frame_bytes: u32) -> Door {
        Door {
            mem,
            receive_at,
            send_at,
            frame_bytes,
            tags: Cell::new(0),
        }
    }

    /// The core's memory, in which an import names what it lends.
    pub fn mem(&self) -> Memory {
        self.mem
    }

    /// Lays a frame in the send frame, with the packet `write` writes, or
    /// none without `write`. A packet that does not fit answers EMSGSIZE.
    fn lay(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        write: Option<Write>,
    ) -> Result<(), Errno> {
        let pkt_len = match write {
            Some(write) => {
                // SAFETY: the send frame's room past its words, which only the
                // door writes, and the runtime reads only inside `send` or
                // `call`.
                let room = unsafe {
                    core::slice::from_raw_parts_mut(
                        self.mem.at::<u8>((self.send_at + WORD_BYTES) as usize),
                        (self.frame_bytes - WORD_BYTES) as usize,
                    )
                };
                write(room).ok_or(Errno::EMSGSIZE)?
            }
            None => 0,
        };
        let f = Frame {
            tag,
            cmd,
            flags,
            result,
            pkt_len,
        };
        f.lay(self.mem, self.send_at);
        Ok(())
    }

    /// Sends a frame with the packet `write` writes, or none, and the memory
    /// it names. A frame that does not fit is a bug.
    fn send_frame(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        named: (u32, u32),
        write: Option<Write>,
    ) {
        let laid = self.lay(tag, cmd, flags, result, write);
        xpute_kit::ensure!(laid.is_ok(), EMSGSIZE, cmd);
        host::send(self.send_at, named.0, named.1);
    }

    /// Sends a frame tagged `tag` and flagged `flags`, with `result` and no
    /// packet.
    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32) {
        self.send_frame(tag, cmd, flags, result, (0, 0), None);
    }

    pub fn send<E: Import>(&self, import: &E) {
        self.import(0, import);
    }

    /// Sends `import` tagged `tag`, with ACKREQ where it goes with it, for the
    /// runtime to complete on a later turn.
    pub fn import<E: Import>(&self, tag: u32, import: &E) {
        let write = packet_of(import);
        let flags = if E::ACKREQ { frame::ACKREQ } else { 0 };
        self.send_frame(tag, E::NUMBER, flags, 0, import.named(), Some(&write));
    }

    /// `call` sends `import`, which the runtime answers before this returns.
    /// It answers the runtime's value or errno, or EMSGSIZE when the frame
    /// does not fit.
    pub fn call<E: Import>(&self, import: &E) -> Result<u32, Errno> {
        self.lay(0, E::NUMBER, frame::ACKREQ, 0, Some(&packet_of(import)))?;
        answer_of(host::call(self.send_at))
    }

    /// The next handle with no value, slot 0 under its next generation: the
    /// handle of a request, or of a written file, which holds no slot of a
    /// table (abi/handle.rs). It differs from every handle that holds a value,
    /// and from those it issued before, until its generation comes round.
    pub fn tag(&self) -> u32 {
        let generation = self.tags.get().wrapping_add(1).max(1);
        self.tags.set(generation);
        tag_of(generation)
    }

    /// Sends `import` with ACKREQ under the next handle with no value, and
    /// answers it: the runtime completes the request on a later turn under
    /// that handle (`Event::Completion`).
    pub fn request<E: Import>(&self, import: &E) -> u32 {
        let handle = self.tag();
        self.import(handle, import);
        handle
    }

    /// Sends the stop report as the core stops. Where the whole report would
    /// not fit in a frame, its file path is cut to its last bytes, or left
    /// out.
    pub fn abort(&self, errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
        let path = at.file();
        let tail = |n: usize| path.get(path.len().saturating_sub(n)..).unwrap_or("");
        for file in [path, tail(ABORT_PATH_BYTES), ""] {
            let report = SysAbort {
                errnum: errnum as i32,
                line: at.line(),
                column: at.column(),
                a_low: a as u32,
                a_high: (a >> 32) as u32,
                b_low: b as u32,
                b_high: (b >> 32) as u32,
                file: &file,
            };
            if self
                .lay(0, SysAbort::NUMBER, 0, 0, Some(&packet_of(&report)))
                .is_ok()
            {
                return host::send(self.send_at, 0, 0);
            }
        }
    }

    /// Receives and applies every frame that waits, in order. It replies only
    /// to frames flagged `ACKREQ` and to failures, and never to a completion.
    pub fn receive(&self, mut handle: impl FnMut(Event) -> Outcome) {
        while host::receive(self.receive_at, self.frame_bytes) > 0 {
            let f = Frame::at(self.mem, self.receive_at);
            let pkt = f.pkt(self.mem, self.receive_at, self.frame_bytes);
            if f.flags & frame::RES != 0 {
                let _ = handle(Event::Completion(Completion {
                    cmd: f.cmd,
                    tag: f.tag,
                    res: f.result,
                    pkt: pkt.ok().flatten(),
                }));
                continue;
            }
            let result = match pkt {
                Ok(pkt) => match apply_frame(&f, pkt, &mut handle) {
                    Ok(Some(reply)) => Ok(reply),
                    Ok(None) => Err(Errno::ENOSYS as i32),
                    Err(e) => Err(e.errnum as i32),
                },
                Err(e) => Err(e.errnum as i32),
            };
            match result {
                Ok(r) if f.flags & frame::ACKREQ != 0 => {
                    let named = (self.mem.off_of_slice(r.named) as u32, r.named.len() as u32);
                    self.send_frame(f.tag, f.cmd, frame::RES, r.val, named, None);
                }
                Ok(_) => {}
                Err(errnum) => self.post(f.tag, f.cmd, frame::RES, errnum),
            }
        }
    }
}

/// The writer of `import`'s packet into the room it is given.
fn packet_of<E: Import>(import: &E) -> impl Fn(&mut [u8]) -> Option<u32> + '_ {
    move |out| {
        let mut w = PacketWriter::new(out, E::FIELDS);
        import.write(&mut w);
        w.fit_len()
    }
}

/// The door's end of xpute's own protocols: NOP answered here, and every
/// other export handed to the program as what it means.
struct Own<'h, F> {
    handle: &'h mut F,
}

impl<F: FnMut(Event) -> Outcome> sys::Exports for Own<'_, F> {
    fn nop(&mut self, c: Nop) -> Outcome {
        Ok(Some((c.val as i32).into()))
    }
    fn sys_attach(&mut self) -> Outcome {
        (self.handle)(Event::Life(Life::Attach))
    }
    fn sys_suspend(&mut self) -> Outcome {
        (self.handle)(Event::Life(Life::Suspend))
    }
    fn sys_resume(&mut self) -> Outcome {
        (self.handle)(Event::Life(Life::Resume))
    }
    fn sys_turn(&mut self, c: SysTurn) -> Outcome {
        (self.handle)(Event::Turn(c))
    }
    fn sys_frame(&mut self, c: SysFrame) -> Outcome {
        (self.handle)(Event::Frame(c))
    }
}

impl<F: FnMut(Event) -> Outcome> fs::Exports for Own<'_, F> {
    fn fs_changed(&mut self, c: FsChanged<'_>) -> Outcome {
        (self.handle)(Event::Changed(c))
    }
}

/// Applies one submitted command, by its major: xpute's own through their
/// protocols' generated `apply`, and every command past xpute's majors to the
/// program. A number xpute gives no protocol is answered with nothing.
fn apply_frame(
    f: &Frame,
    pkt: Option<&[u8]>,
    handle: &mut impl FnMut(Event) -> Outcome,
) -> Outcome {
    let own = &mut Own { handle };
    match frame::cmd_major(f.cmd) {
        m if m == Major::NOP as u32 || m == Major::SYS as u32 => sys::apply(own, f.cmd, &pkt),
        m if m == fs::Major::FS as u32 => fs::apply(own, f.cmd, &pkt),
        m if m < PROGRAM_MAJOR => Ok(None),
        _ => (own.handle)(Event::Program(f.cmd, pkt)),
    }
}

static DOOR: Global<Option<Door>> = Global::new();

/// Checks the front's header the runtime wrote against `m`, once, on the
/// first turn, and opens the door over two frames on the heap, which the door
/// holds for the core's life. Where a word of the header differs from the
/// manifest's, the core stops with ENOEXEC, the word's place and what it holds.
/// Every later call finds the door open and does nothing.
pub fn boot(m: &Manifest) {
    if booted() {
        return;
    }
    let mem = crate::mem::core();
    // SAFETY: the front's header page, which the runtime wrote before the turn.
    let differs = unsafe { head::differs(mem, m.head_at() as usize, &m.head()) };
    // Whole words, so that each frame and its packet lie 8-aligned.
    let words = (m.frame_bytes() / 8) as usize;
    let frames: &'static mut [u64] = Vec::leak(vec![0; 2 * words]);
    let (receive, send) = frames.split_at_mut(words);
    let at = |frame: &[u64]| off_of_slice(frame) as u32;
    set(Door::new(mem, at(receive), at(send), m.frame_bytes()));
    if let Some((at, found)) = differs {
        door().abort(Errno::ENOEXEC, Location::caller(), at.into(), found.into());
        xpute_kit::bug!(ENOEXEC, at, found);
    }
}

/// Makes `door` the core's, as `boot` does, and as a core does that lays its
/// frames out itself.
pub fn set(door: Door) {
    DOOR.set(Some(door));
}

/// This is the door `boot` or `set` opened. Asking for it before is a bug.
pub fn door() -> &'static Door {
    use xpute_kit::status::bug::OrBug;
    DOOR.get(|| None).as_ref().or_bug(Errno::ENOTRECOVERABLE)
}

pub fn booted() -> bool {
    DOOR.get(|| None).is_some()
}

#[cfg(test)]
#[path = "door.test.rs"]
pub(crate) mod test;
