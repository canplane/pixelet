// xpute-conformance/core.rs

//! The Rust conformance core. It exports the same two calls as the C++
//! conformance core, so that one runtime drives both. `boot` takes a seed,
//! lays out the parts of the front by it, writes the front's header as a
//! runtime does for a machine, and answers the header's offset. Natively,
//! the runtime hands the core a region, a clock and its call. As wasm, the
//! core imports all three, its range starts past the linker's data, and its
//! turn is the module's `run`.

#[cfg(not(target_arch = "wasm32"))]
use core::cell::Cell;

use xpute_core::abi::handle::Table;
use xpute_core::ipc::door::{Door, Import, Outcome, Submission};
use xpute_core::ipc::file::Files;
use xpute_core::ipc::head::{self, Head};
use xpute_core::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_core::ipc::stream::Stream;
use xpute_core::mem::Memory;
use xpute_kit::math::rng::SplitMix32;
use xpute_kit::math::scalar::round;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

use crate::spec;

use xpute_core::mem::section::PAGE_BYTES;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

// The parts the core shares with its runtime, each on pages of its own, which
// `layout` places in an order the seed draws.
const HEAD: usize = 0;
const SQ_DESC: usize = 1;
const CQ_DESC: usize = 2;
const SQ_SLOT: usize = 3;
const CQ_SLOT: usize = 4;
const STREAM: usize = 5;
const PARTS: usize = 6;

const PART_BYTES: [u32; PARTS] = [
    head::spec::WORDS * 4,
    ring_bytes(spec::SQ_CAP),
    ring_bytes(spec::CQ_CAP),
    slots_bytes(spec::SQ_CAP, spec::SLOT_BYTES),
    slots_bytes(spec::CQ_CAP, spec::SLOT_BYTES),
    spec::STREAM_WORDS * 4,
];

/// The most any layout spans: the first page, which it starts past, every
/// part, and the most gap pages before each part. The core's own state lies
/// here, past every layout, so that no layout moves it.
const STATE_AT: u32 = {
    let mut end = PAGE_BYTES + PARTS as u32 * spec::GAP_PAGES * PAGE_BYTES;
    let mut p = 0;
    while p < PARTS {
        end += page_up(PART_BYTES[p]);
        p += 1;
    }
    end
};

/// Where the heap begins, which only the wasm build uses; a native build
/// allocates from the process.
const HEAP_AT: u32 = page_up(STATE_AT + core::mem::size_of::<Core>() as u32);

const _: () = assert!(
    HEAP_AT as usize + spec::HEAP_BYTES as usize <= spec::MEM_BYTES as usize,
    "the ranges run past the memory"
);
const _: () = assert!(
    (STATE_AT as usize).is_multiple_of(core::mem::align_of::<Core>()),
    "the state lies on its own alignment"
);

pub type Now = extern "C" fn() -> f64;
pub type Call = extern "C" fn(u32) -> i32;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    /// The native runtime's call, which `boot` installs as the core's.
    static CALL: Cell<Option<Call>> = const { Cell::new(None) };
}

#[cfg(not(target_arch = "wasm32"))]
fn call(at: u32) -> i32 {
    CALL.with(Cell::get).map_or(Errno::ENOSYS as i32, |f| f(at))
}

struct Core {
    door: Door,
    state: State,
}

struct State {
    stream: Stream,
    files: Files<u32, { spec::HANDLES as usize }>,
    handles: Table<(), { spec::HANDLES as usize }>,
    now: Now,
    /// The clock at the last MARK.
    mark: f64,
    /// What the runtime writes as it answers CALL's INC.
    word: u32,
}

/// The import CALL makes: a number, and the offset of the word the runtime
/// writes in answer.
struct Inc {
    n: u32,
    at: u32,
}

impl Import for Inc {
    const NUMBER: u32 = spec::cmd::INC;
    const FIELDS: u32 = 2;
    fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.n));
        w.u32(Some(self.at));
    }
}

/// Where each part lies under `seed`, counted from the range's start. The
/// seed draws the order of the parts and, for each part, how many pages to
/// leave before it. The first page is never used, since offset 0 names
/// nothing (ARCHITECTURE.md, the contract's rule 1).
fn layout(seed: u32) -> [u32; PARTS] {
    let mut rng = SplitMix32::new(seed);
    let mut order: [usize; PARTS] = core::array::from_fn(|p| p);
    for i in (1..PARTS).rev() {
        order.swap(i, (rng.next_u32() % (i as u32 + 1)) as usize);
    }
    let mut at = [0; PARTS];
    let mut end = PAGE_BYTES;
    for p in order {
        end += rng.next_u32() % (spec::GAP_PAGES + 1) * PAGE_BYTES;
        at[p] = end;
        end += page_up(PART_BYTES[p]);
    }
    at
}

/// Every offset the core writes is counted from the memory's start, so
/// `range`, where the core's range starts, is added to each.
///
/// # Safety
/// The range must be `spec::MEM_BYTES` long and inside the memory, which
/// outlives the core, and nothing else may write it while a call runs.
unsafe fn boot(mem: Memory, range: u32, now: Now, seed: u32) -> u32 {
    let at = layout(seed).map(|off| range + off);
    // SAFETY: the caller's memory; the ranges are the layout's.
    let (sq, cq, stream) = unsafe {
        (
            Ring::init(
                mem,
                at[SQ_DESC] as usize,
                spec::SQ_CAP,
                at[SQ_SLOT],
                spec::SLOT_BYTES,
            ),
            Ring::init(
                mem,
                at[CQ_DESC] as usize,
                spec::CQ_CAP,
                at[CQ_SLOT],
                spec::SLOT_BYTES,
            ),
            Stream::new(
                mem,
                at[STREAM] as usize,
                spec::STREAM_WORDS as usize,
                Stream::OWN_WORDS,
            ),
        )
    };
    let door = Door::new(sq, cq, spec::MOST_RAISE);
    // The stream lies where a GPU stream would, so the runtime runs it after
    // every turn.
    let front = Head {
        sq_cap: spec::SQ_CAP,
        cq_cap: spec::CQ_CAP,
        slot_bytes: spec::SLOT_BYTES,
        sq_at: at[SQ_DESC],
        cq_at: at[CQ_DESC],
        gpu_at: at[STREAM],
    };
    // SAFETY: as above.
    unsafe { head::write(mem, at[HEAD] as usize, &front) };
    let core = Core {
        door,
        state: State {
            stream,
            files: Files::new(),
            handles: Table::new(),
            now,
            mark: 0.0,
            word: 0,
        },
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { mem.at::<Core>((range + STATE_AT) as usize).write(core) };
    at[HEAD]
}

/// # Safety
/// `boot` must have laid out a core at `range` in `mem`.
unsafe fn turn(mem: Memory, range: u32) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let Core { door, state } = unsafe { &mut *mem.at::<Core>((range + STATE_AT) as usize) };
    let wake = door.turn(|submission| match submission {
        Submission::Opened(opened) => {
            if let Some(owner) = state.files.complete_open(&opened) {
                door.post(owner, spec::cmd::OPENED, 0, opened.res);
            }
            Ok(Some(0))
        }
        Submission::Program(cmd, pkt) => apply(mem, door, state, cmd, &pkt),
        // The conformance core keeps nothing across its life, reads no file
        // again, and draws nothing, so it answers 0 to every other
        // submission, and a surface it is lent gets the handle 0, none.
        _ => Ok(Some(0)),
    });
    state.stream.publish();
    wake
}

/// # Safety
/// `mem` must be `spec::MEM_BYTES` long, 8-aligned and outlive the core, and
/// nothing else may write it while a call runs.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(
    mem: *mut u8,
    now: Now,
    runtime: Call,
    seed: u32,
) -> u32 {
    CALL.with(|h| h.set(Some(runtime)));
    xpute_core::call::set(call);
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(given(mem), 0, now, seed) }
}

/// # Safety
/// `mem` must be a memory over which `xpute_conformance_boot` laid out a
/// core.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_run(mem: *mut u8, _deadline_ms: f64) -> f64 {
    // SAFETY: the caller's memory, booted.
    unsafe { turn(given(mem), 0) }
}

/// The memory a native runtime hands over: `spec::MEM_BYTES` from `mem`.
#[cfg(not(target_arch = "wasm32"))]
fn given(mem: *mut u8) -> Memory {
    // SAFETY: the runtime hands over `spec::MEM_BYTES` from `mem`, kept for the core's life.
    unsafe { Memory::new(mem.expose_provenance(), spec::MEM_BYTES as usize) }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use xpute_core::entry::BootInfo;
    use xpute_core::wasm::Config;

    // The heap is the last part of the range, from `HEAP_AT` to the memory's
    // end.
    xpute_core::wasm_abi!(
        Config {
            heap_at: |data_end| range_at(data_end) + HEAP_AT,
            stack_bytes: spec::STACK_BYTES,
            admit_frame: None,
        },
        entry
    );

    extern "C" fn now() -> f64 {
        xpute_core::wasm::now()
    }

    /// The range starts past where the linker's data ends.
    fn range_at(data_end: u32) -> u32 {
        page_up(data_end)
    }

    #[no_mangle]
    extern "C" fn xpute_conformance_boot(seed: u32) -> u32 {
        let range = range_at(xpute_core::wasm::data_end());
        // A range past the memory's end means a runtime sized the memory for
        // another core.
        xpute_kit::ensure!(
            range as u64 + spec::MEM_BYTES as u64 <= xpute_core::wasm::given(),
            ENOMEM,
            range
        );
        // SAFETY: the module's memory, from address 0, and the range inside
        // it past what the linker placed.
        unsafe { boot(xpute_core::mem::core(), range, now, seed) }
    }

    fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
        // SAFETY: the module's memory, booted.
        unsafe {
            turn(
                xpute_core::mem::core(),
                range_at(xpute_core::wasm::data_end()),
            )
        }
    }
}

fn apply(mem: Memory, door: &Door, state: &mut State, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
    let State {
        stream,
        files,
        handles,
        now,
        mark,
        word,
    } = state;
    let bad_handle = || MarshalError::new(Errno::EBADF);
    match cmd {
        spec::cmd::OK => Ok(Some(0)),
        spec::cmd::FAIL => Err(MarshalError::new(Errno::EINVAL)),
        spec::cmd::RAISE => {
            let n = arg(pkt)?;
            if n > spec::MOST_RAISE {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            for k in 0..n {
                door.post(k, spec::cmd::NOTE, 0, 0);
            }
            Ok(Some(n as i32))
        }
        spec::cmd::RECORD => {
            stream.record(arg(pkt)?, &[], &[], None);
            Ok(Some(0))
        }
        spec::cmd::ISSUE => Ok(Some(
            handles
                .insert(())
                .ok_or_else(|| MarshalError::new(Errno::ENFILE))? as i32,
        )),
        spec::cmd::RELEASE => handles
            .remove(arg(pkt)?)
            .map(|()| Some(0))
            .ok_or_else(bad_handle),
        spec::cmd::USE => handles
            .live(arg(pkt)?)
            .then_some(Some(0))
            .ok_or_else(bad_handle),
        spec::cmd::MARK => {
            *mark = now();
            Ok(Some(0))
        }
        spec::cmd::SINCE => Ok(Some(round(now() - *mark) as i32)),
        spec::cmd::OPEN => Ok(Some(files.open(door, 1, "f", arg(pkt)?) as i32)),
        spec::cmd::CLOSE => files
            .close(door, arg(pkt)?)
            .map(|_| Some(0))
            .ok_or_else(bad_handle),
        spec::cmd::CALL => {
            let n = arg(pkt)?;
            *word = 0;
            let at = mem.off_of(&raw const *word) as u32;
            door.call(&Inc { n, at }).map_err(MarshalError::new)?;
            // SAFETY: the word the runtime wrote before the call returned.
            Ok(Some(unsafe { (&raw const *word).read_volatile() } as i32))
        }
        _ => Ok(None),
    }
}

fn arg(pkt: &Option<&[u8]>) -> Result<u32, MarshalError> {
    let bytes = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    Ok(TreeReader::new(bytes)?.read_branch()?.at(0)?.get_number()? as u32)
}
