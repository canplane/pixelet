// xpute-guest/program.rs

//! A guest in one declaration (`program!`): its range laid out past where
//! the linker stopped, the toc's page, the two rings and their slots and the
//! hostcalls' stream, then the heap; its turns run; and each command handed
//! to the function the program names for it, whose result is the reply and
//! whose errno is the failure. Work that waits on the host runs as a task
//! (`Turn::spawn`): a file read whole (`read`) is awaited, and each await ends
//! at a later turn. A bug or a panic is published to the host (`stop`). What
//! lies under it, `ipc::door`, `ipc::sys`, `ipc::toc`, `sched::task` and
//! `stop`, a guest that lays its memory out itself uses as it is.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, PacketWriter, TreeReader};

use crate::abi::cmd::SysOpened;
use crate::global::Global;
use crate::ipc::door::{Door, Event, Submission};
use crate::ipc::file::Files;
use crate::ipc::ring::{ring_bytes, slots_bytes, Ring};
use crate::ipc::sys::{spec::op, Fd, Sys};
use crate::ipc::toc::{spec::key, toc_words, write_toc};
use crate::mem::section::{page_up, PAGE_BYTES};
use crate::sched::task::{self, Executor, Task};
use crate::sched::wake::NO_WAKE;
use crate::stop;

#[path = "program.spec.rs"]
pub mod spec;

/// The port a program's host speaks as unless it says otherwise, and the one
/// a handler's events are raised for (@xpute/host port.ts `PROGRAM_PORT`).
pub const PROGRAM_PORT: u32 = spec::PORT;

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
        cap: spec::CAP,
        slot_bytes: spec::SLOT_BYTES,
        sys_words: spec::SYS_WORDS,
        most_events: spec::MOST_EVENTS,
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
        page_up(self.sys_at() + 4 * self.sys_words)
    }
}

/// The range starts on the page past where the linker stopped.
pub const fn range_at(data_end: u32) -> u32 {
    page_up(data_end)
}

/// Where the heap begins in the memory (`wasm::Config::heap_at`).
pub const fn heap_at(layout: &Layout, data_end: u32) -> u32 {
    range_at(data_end) + layout.heap_at()
}

/// The files a program may have open at once, each a read awaited.
const MOST_FILES: usize = spec::MOST_FILES as usize;

struct Program {
    door: Door,
    sys: Sys,
    files: Files<(), MOST_FILES>,
    reads: Vec<Reading>,
    /// Turns run since boot.
    turn: u32,
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
        files: Files::new(),
        reads: Vec::new(),
        turn: 0,
    }));
    task::set(&TASKS);
    stop::install(abort, None);
    range
}

/// A stop published as the turn's one record.
fn abort(errnum: Errno, at: &core::panic::Location<'_>, a: u64, b: u64) {
    if let Some(p) = PROGRAM.get(|| None) {
        p.sys.abort(errnum, at, a, b);
    }
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

/// What a handler takes a command's packet as: the packet itself, or the
/// struct its table generates for it (xpute-tool `spec/cmd`), read here, and
/// EBADMSG where it does not read.
pub trait FromPacket<'a>: Sized {
    fn from_packet(pkt: Packet<'a>) -> Result<Self, Errno>;
}

impl<'a> FromPacket<'a> for Packet<'a> {
    fn from_packet(pkt: Packet<'a>) -> Result<Self, Errno> {
        Ok(pkt)
    }
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

    /// An event for the program's port, heard after the turn.
    pub fn raise<E: Event>(&self, event: &E) {
        self.door.raise_to(PROGRAM_PORT, event);
    }

    /// An event of no table's: `cmd`, and `cnt` fields, which `f` writes.
    pub fn raise_packet(&self, cmd: u32, cnt: u32, f: impl Fn(&mut PacketWriter)) {
        self.door.raise(PROGRAM_PORT, cmd, cnt, f);
    }

    /// Work that waits on the host, first polled after this turn's commands
    /// and then whenever what it awaits is ready; EAGAIN where the executor
    /// has no room.
    pub fn spawn(&self, work: impl Future<Output = ()> + 'static) -> Result<(), Errno> {
        if task::spawn(work, 0) {
            Ok(())
        } else {
            Err(Errno::EAGAIN)
        }
    }
}

/// An event for the program's port from a task, heard after the turn.
pub fn raise<E: Event>(event: &E) {
    if let Some(p) = PROGRAM.get(|| None) {
        p.door.raise_to(PROGRAM_PORT, event);
    }
}

/// The file at `path` below `root`, whole: its bytes, or the errno its open
/// was refused with (ENOENT where there is none; what a root means is the
/// host's `Fs`), EMFILE where as many are open as a program may have, ENOMEM
/// where the heap has no room for it. Opened on the turn it is first polled,
/// read on the turn its open is answered, and ready on the turn after.
pub fn read(root: u32, path: &str) -> Read {
    Read {
        root,
        path: path.into(),
        fd: None,
    }
}

/// A read awaited (`read`); dropped before it is ready, its open is cancelled.
pub struct Read {
    root: u32,
    path: Box<str>,
    fd: Option<Fd>,
}

enum Stage {
    Opening,
    /// Filled by the host after the turn `asked_turn` ends.
    Reading {
        bytes: Vec<u8>,
        asked_turn: u32,
    },
    Done(Result<Vec<u8>, Errno>),
}

struct Reading {
    fd: Fd,
    got: Stage,
    waker: Option<Waker>,
    /// Its `Read` was dropped: kept only while the host may still fill it.
    gone: bool,
}

impl Reading {
    fn ready(&mut self, res: Result<Vec<u8>, Errno>) {
        self.got = Stage::Done(res);
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }
}

impl Future for Read {
    type Output = Result<Vec<u8>, Errno>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(p) = PROGRAM.get(|| None) else {
            return Poll::Ready(Err(Errno::ENXIO));
        };
        let Some(fd) = this.fd else {
            let Some(fd) = p.files.try_open(&mut p.sys, this.root, &this.path, ()) else {
                return Poll::Ready(Err(Errno::EMFILE));
            };
            this.fd = Some(fd);
            p.reads.push(Reading {
                fd,
                got: Stage::Opening,
                waker: Some(cx.waker().clone()),
                gone: false,
            });
            return Poll::Pending;
        };
        let Some(i) = p.reads.iter().position(|r| r.fd == fd && !r.gone) else {
            return Poll::Ready(Err(Errno::EBADF));
        };
        if let Stage::Done(_) = p.reads[i].got {
            this.fd = None;
            let Stage::Done(res) = p.reads.swap_remove(i).got else {
                unreachable!()
            };
            return Poll::Ready(res);
        }
        p.reads[i].waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for Read {
    fn drop(&mut self) {
        let (Some(fd), Some(p)) = (self.fd, PROGRAM.get(|| None)) else {
            return;
        };
        let Some(i) = p.reads.iter().position(|r| r.fd == fd && !r.gone) else {
            return;
        };
        match p.reads[i].got {
            Stage::Opening => {
                p.files.close(&mut p.sys, fd);
                p.reads.swap_remove(i);
            }
            Stage::Reading { .. } => p.reads[i].gone = true,
            Stage::Done(_) => {
                p.reads.swap_remove(i);
            }
        }
    }
}

impl Program {
    /// An open answered: an errno ends its read, a size has the host fill a
    /// buffer of it after this turn.
    fn complete_open(&mut self, opened: &SysOpened) {
        if self.files.complete_open(&mut self.sys, opened).is_none() {
            return;
        }
        let Some(r) = self.reads.iter_mut().find(|r| r.fd == opened.fd) else {
            return;
        };
        if opened.res < 0 {
            r.ready(Err(Errno::of(opened.res).unwrap_or(Errno::EIO)));
            return;
        }
        // SAFETY: the buffer is kept in `reads`, never grown, until a later
        // turn's rise; nothing reads it before then.
        let read = unsafe {
            self.files
                .read_whole(&mut self.sys, opened.fd, opened.res as u32)
        };
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(e) => return r.ready(Err(e)),
        };
        r.got = Stage::Reading {
            bytes,
            asked_turn: self.turn,
        };
    }

    /// The reads the host filled after an earlier turn, ready; those whose
    /// `Read` was dropped, let go.
    fn rise(&mut self) {
        self.turn += 1;
        let turn = self.turn;
        self.reads
            .retain(|r| !(r.gone && turn > asked_of(&r.got).unwrap_or(0)));
        for r in &mut self.reads {
            if asked_of(&r.got).is_some_and(|asked| turn > asked) {
                let Stage::Reading { bytes, .. } = core::mem::replace(&mut r.got, Stage::Opening)
                else {
                    unreachable!()
                };
                r.ready(Ok(bytes));
            }
        }
    }

    /// A read waits for the next turn.
    fn waiting(&self) -> bool {
        self.reads
            .iter()
            .any(|r| matches!(r.got, Stage::Reading { .. }))
    }
}

fn asked_of(got: &Stage) -> Option<u32> {
    match got {
        Stage::Reading { asked_turn, .. } => Some(*asked_turn),
        _ => None,
    }
}

/// The executor `program!` installs: each task polled once a turn while it is
/// woken, a woken one asking for the next turn. Another installed in its place
/// (`sched::task::set`) runs the same tasks.
struct Tasks;

struct TaskList {
    slots: Vec<Option<Task>>,
    woken: Vec<bool>,
}

static TASK_LIST: Global<TaskList> = Global::new();
static TASKS: Tasks = Tasks;

fn task_list() -> &'static mut TaskList {
    TASK_LIST.get(|| TaskList {
        slots: Vec::new(),
        woken: Vec::new(),
    })
}

const WAKER: RawWakerVTable = RawWakerVTable::new(
    |data| RawWaker::new(data, &WAKER),
    |data| woke(data.addr()),
    |data| woke(data.addr()),
    |_| {},
);

fn woke(slot: usize) {
    if let Some(w) = task_list().woken.get_mut(slot) {
        *w = true;
    }
}

impl Executor for Tasks {
    fn spawn(&self, work: Task, _priority: task::Priority) -> bool {
        let list = task_list();
        let slot = list
            .slots
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                list.slots.push(None);
                list.woken.push(false);
                list.slots.len() - 1
            });
        list.slots[slot] = Some(work);
        list.woken[slot] = true;
        true
    }

    fn poll(&self) {
        let len = task_list().slots.len();
        for slot in 0..len {
            let list = task_list();
            if !core::mem::take(&mut list.woken[slot]) {
                continue;
            }
            let Some(mut work) = list.slots[slot].take() else {
                continue;
            };
            // SAFETY: the vtable's calls take only the slot's number, no memory.
            let waker = unsafe {
                Waker::from_raw(RawWaker::new(core::ptr::without_provenance(slot), &WAKER))
            };
            if work
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
            {
                task_list().slots[slot] = Some(work);
            } else {
                // SAFETY: done, so never polled again; unpinned only to be freed.
                let done: &'static mut (dyn Future<Output = ()> + 'static) =
                    unsafe { Pin::into_inner_unchecked(work) };
                let raw: *mut (dyn Future<Output = ()> + 'static) = done;
                // SAFETY: `task::spawn` made it a box and handed it here once.
                drop(unsafe { Box::from_raw(raw) });
            }
        }
    }

    fn pending(&self) -> bool {
        task_list().woken.iter().any(|w| *w)
    }
}

/// What a command is handed to: its reply, or its errno.
pub type Handler = fn(&mut Turn<'_>, Packet<'_>) -> Result<i32, Errno>;

/// A turn: what waits, each command to its handler in order, ENOSYS for one
/// with none; the next turn asked for while any is left.
pub fn turn(handlers: &[(u32, Handler)]) -> f64 {
    let Some(p) = PROGRAM.get(|| None) else {
        return NO_WAKE;
    };
    p.sys.rise();
    p.rise();
    let mut answered: Vec<SysOpened> = Vec::new();
    {
        let Program { door, sys, .. } = &mut *p;
        door.drain(|s| match s {
            Submission::Opened(o) => {
                answered.push(o);
                Ok(Some(0))
            }
            Submission::Program(cmd, pkt) => match handlers.iter().find(|(c, _)| *c == cmd) {
                None => Ok(None),
                Some((_, handle)) => handle(&mut Turn { sys, door }, Packet(pkt))
                    .map(Some)
                    .map_err(MarshalError::new),
            },
        });
    }
    for o in &answered {
        p.complete_open(o);
    }
    // The tasks reach the program through `PROGRAM` themselves.
    task::poll();
    let Some(p) = PROGRAM.get(|| None) else {
        return NO_WAKE;
    };
    p.sys.fall();
    if p.door.pending() || p.waiting() || task::pending() {
        0.0
    } else {
        NO_WAKE
    }
}

/// A guest built as a WebAssembly module, in one declaration: its layout
/// (`Layout::DEFAULT` unless given), the stack it was linked with (as its
/// build.rs's `wasm::link` was told; `LinkConfig::DEFAULT`'s unless given),
/// and a handler for each command, which takes its packet as `Packet` or as
/// the struct its table generates (`FromPacket`). It exports `run`, the turn,
/// and `boot`, which lays the range out and answers where the toc lies
/// (@xpute/host port.ts `open`).
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
            $crate::program::turn(&[$((
                $cmd as u32,
                (|cx: &mut $crate::program::Turn<'_>, pkt: $crate::program::Packet<'_>| {
                    $handler(cx, $crate::program::FromPacket::from_packet(pkt)?)
                }) as $crate::program::Handler,
            )),*])
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
