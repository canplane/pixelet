// xpute-guest/program.rs

//! A guest in one declaration (`program!`): its range laid out past where
//! the linker stopped, the toc's page, the two rings and their slots and the
//! hostcalls' stream, then the heap; its turns run; and each command handed
//! to the function the program names for it, whose result is the reply and
//! whose errno is the failure. What lies under it, `ipc::door`, `ipc::sys`
//! and `ipc::toc`, a guest that lays its memory out itself uses as it is.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, PacketWriter, TreeReader};

use crate::global::Global;
use crate::ipc::door::{Door, Submission};
use crate::ipc::ring::{ring_bytes, slots_bytes, Ring};
use crate::ipc::sys::{spec::op, Sys};
use crate::ipc::toc::{spec::key, toc_words, write_toc};
use crate::mem::section::PAGE_BYTES;
use crate::sched::wake::NO_WAKE;

/// The port a program's host speaks as unless it says otherwise, and the one
/// a handler's events are raised for (@xpute/host port.ts `PROGRAM_PORT`).
pub const PROGRAM_PORT: u32 = 1;

/// What the range holds, by count; the offsets follow from it.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Each ring's slots: how many commands can wait, and replies and events
    /// be left, between two turns.
    pub cap: u32,
    /// A slot's bytes: the most a command's packet, or an event's, may be.
    pub slot_bytes: u32,
    /// The hostcalls' stream, in words: what one turn may ask the host to run
    /// after it.
    pub sys_words: u32,
    /// The events one command may raise beside its reply.
    pub most_events: u32,
}

impl Layout {
    pub const DEFAULT: Layout = Layout {
        cap: 64,
        slot_bytes: 1024,
        sys_words: 4096,
        most_events: 4,
    };

    const fn sq_at(&self) -> u32 {
        PAGE_BYTES
    }
    const fn cq_at(&self) -> u32 {
        self.sq_at() + ring_bytes(self.cap)
    }
    const fn sq_slots_at(&self) -> u32 {
        self.cq_at() + ring_bytes(self.cap)
    }
    const fn cq_slots_at(&self) -> u32 {
        self.sq_slots_at() + slots_bytes(self.cap, self.slot_bytes)
    }
    const fn sys_at(&self) -> u32 {
        self.cq_slots_at() + slots_bytes(self.cap, self.slot_bytes)
    }

    /// Where the heap begins, from the range's start.
    pub const fn heap_at(&self) -> u32 {
        (self.sys_at() + 4 * self.sys_words).next_multiple_of(PAGE_BYTES)
    }
}

/// The range starts on the page past where the linker stopped.
pub const fn range_at(data_end: u32) -> u32 {
    data_end.next_multiple_of(PAGE_BYTES)
}

/// Where the heap begins in the memory (`wasm::Config::heap_at`).
pub const fn heap_at(layout: &Layout, data_end: u32) -> u32 {
    range_at(data_end) + layout.heap_at()
}

struct Program {
    door: Door,
    sys: Sys,
}

static PROGRAM: Global<Option<Program>> = Global::new();

/// Lays the range out and writes the toc, before the first turn: the toc's
/// offset, which the module's `boot` answers.
pub fn boot(layout: &Layout, data_end: u32) -> u32 {
    let mem = crate::mem::core();
    let range = range_at(data_end);
    let at = |off: u32| range + off;
    // SAFETY: the range past what the linker placed and short of the heap,
    // laid out once, before anything else reads it.
    let (sq, cq, sys) = unsafe {
        (
            Ring::init(
                mem,
                at(layout.sq_at()) as usize,
                layout.cap,
                at(layout.sq_slots_at()),
                layout.slot_bytes,
            ),
            Ring::init(
                mem,
                at(layout.cq_at()) as usize,
                layout.cap,
                at(layout.cq_slots_at()),
                layout.slot_bytes,
            ),
            Sys::new(mem, at(layout.sys_at()) as usize, layout.sys_words as usize),
        )
    };
    // SAFETY: as above; the toc's page is its own.
    unsafe {
        write_toc(
            mem,
            range as usize,
            toc_words(3),
            &[
                (key::SQ, at(layout.sq_at())),
                (key::CQ, at(layout.cq_at())),
                (key::SYS, at(layout.sys_at())),
            ],
        );
    }
    PROGRAM.set(Some(Program {
        door: Door::new(sq, cq, layout.most_events),
        sys,
    }));
    range
}

/// A command's packet as its handler reads it: a branch of fields, or none.
#[derive(Clone, Copy)]
pub struct Packet<'a>(Option<&'a [u8]>);

impl<'a> Packet<'a> {
    /// Its fields; EBADMSG where the command carried none or they do not parse.
    pub fn fields(&self) -> Result<BranchCursor<'a>, Errno> {
        let pkt = self.0.ok_or(Errno::EBADMSG)?;
        TreeReader::new(pkt)
            .and_then(|r| r.read_branch())
            .map_err(errnum)
    }

    /// Field `i` as a number, whatever width the sender lowered it as.
    pub fn number(&self, i: u32) -> Result<f64, Errno> {
        self.fields()?
            .at(i)
            .and_then(|c| c.get_number())
            .map_err(errnum)
    }

    /// Field `i` as text.
    pub fn str(&self, i: u32) -> Result<&'a str, Errno> {
        self.fields()?
            .at(i)
            .and_then(|c| c.get_str_ref())
            .map_err(errnum)
    }

    /// The packet's bytes, for a reader of the program's own.
    pub fn bytes(&self) -> Option<&'a [u8]> {
        self.0
    }
}

fn errnum(e: MarshalError) -> Errno {
    e.errnum
}

/// A turn as a handler sees it: the host to call, and events to raise.
pub struct Turn<'a> {
    sys: &'a mut Sys,
    door: &'a Door,
}

impl Turn<'_> {
    /// A hostcall of the program's own, `code` its number, answered by the
    /// host before it returns from what the host already holds.
    pub fn call_now(&mut self, code: u32, args: &[u32]) -> Result<u32, Errno> {
        self.sys.call_now(code | op::PROGRAM, args, &[])
    }

    /// A hostcall of the program's own, run by the host after the turn: what
    /// it writes back is read on a later turn.
    pub fn call(&mut self, code: u32, args: &[u32], bytes: &[u8]) {
        self.sys.call(code | op::PROGRAM, args, bytes);
    }

    /// An event for the program's port, heard after the turn: `cnt` fields,
    /// which `f` writes.
    pub fn raise(&self, cmd: u32, cnt: u32, f: impl Fn(&mut PacketWriter)) {
        self.door.raise(PROGRAM_PORT, cmd, cnt, f);
    }
}

/// What a command is handed to: its reply, or its errno.
pub type Handler = fn(&mut Turn<'_>, Packet<'_>) -> Result<i32, Errno>;

/// A turn: what waits, each command to its handler in order, ENOSYS for one
/// with none; the next turn asked for while any is left.
pub fn turn(handlers: &[(u32, Handler)]) -> f64 {
    let Some(Program { door, sys }) = PROGRAM.get(|| None) else {
        return NO_WAKE;
    };
    sys.rise();
    door.drain(|s| match s {
        Submission::Opened(_) => Ok(None),
        Submission::Program(cmd, pkt) => match handlers.iter().find(|(c, _)| *c == cmd) {
            None => Ok(None),
            Some((_, handle)) => handle(&mut Turn { sys, door }, Packet(pkt))
                .map(Some)
                .map_err(MarshalError::new),
        },
    });
    sys.fall();
    if door.pending() {
        0.0
    } else {
        NO_WAKE
    }
}

/// A guest built as a WebAssembly module, in one declaration: its layout
/// (`Layout::DEFAULT` unless given), the stack it was linked with (as its
/// build.rs's `wasm::link` was told; `LinkConfig::DEFAULT`'s unless given),
/// and a handler for
/// each command. It exports `run`, the turn, and `boot`, which lays the
/// range out and answers where the toc lies (@xpute/host port.ts `open`).
#[macro_export]
macro_rules! program {
    (layout: $layout:expr, stack_bytes: $stack:expr, $(on $cmd:expr => $handler:expr),* $(,)?) => {
        const XPUTE_LAYOUT: $crate::program::Layout = $layout;

        $crate::wasm_abi!(
            $crate::wasm::Config {
                heap_at: |data_end| $crate::program::heap_at(&XPUTE_LAYOUT, data_end),
                stack_bytes: $stack,
            },
            xpute_turn
        );

        #[no_mangle]
        pub extern "C" fn boot() -> u32 {
            $crate::program::boot(&XPUTE_LAYOUT, $crate::wasm::data_end())
        }

        fn xpute_turn(_quota_ms: f64, _boot: fn() -> $crate::entry::BootInfo) -> f64 {
            $crate::program::turn(&[$(($cmd, $handler as $crate::program::Handler)),*])
        }
    };
    ($(on $cmd:expr => $handler:expr),* $(,)?) => {
        $crate::program!(
            layout: $crate::program::Layout::DEFAULT,
            stack_bytes: $crate::wasm::LinkConfig::DEFAULT.stack_bytes,
            $(on $cmd => $handler),*
        );
    };
}
