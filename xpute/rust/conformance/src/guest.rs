// xpute-conformance/guest.rs

//! The Rust conformance guest, exporting the C++ guest's two calls so one
//! host drives both; `boot` takes a layout's seed and answers the directory's
//! offset. Natively the host hands it a region, a clock and its hostcall; as
//! wasm it imports all three, its range starts past the linker's data, and its
//! turn is the module's `run`.

#[cfg(not(target_arch = "wasm32"))]
use core::cell::Cell;

use xpute_guest::abi::cmd::{Command, SysOpened};
use xpute_guest::abi::handle::Table;
use xpute_guest::ipc::directory::{directory_words, spec::key, write_directory};
use xpute_guest::ipc::door::{Applied, Door};
use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_guest::ipc::stream::Stream;
use xpute_guest::ipc::sys::Sys;
use xpute_guest::mem::Memory;
use xpute_guest::sched::wake::NO_WAKE;
use xpute_kit::math::rng::SplitMix32;
use xpute_kit::math::scalar::round;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::TreeReader;

use crate::spec;

use xpute_guest::mem::section::PAGE_BYTES;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

// What the guest shares with its host, each on pages of its own, in the order
// a layout draws (`layout`).
const DIRECTORY: usize = 0;
const SQ_DESC: usize = 1;
const CQ_DESC: usize = 2;
const SQ_SLOT: usize = 3;
const CQ_SLOT: usize = 4;
const STREAM: usize = 5;
const SYS: usize = 6;
const PARTS: usize = 7;

const PART_BYTES: [u32; PARTS] = [
    directory_words(4) as u32 * 4,
    ring_bytes(spec::SQ_CAP),
    ring_bytes(spec::CQ_CAP),
    slots_bytes(spec::SQ_CAP, spec::SLOT_BYTES),
    slots_bytes(spec::CQ_CAP, spec::SLOT_BYTES),
    spec::STREAM_WORDS * 4,
    spec::SYS_WORDS * 4,
];

/// The most a layout spans: the page it starts past, every part, and the most
/// pages before each. The guest's own state follows, where no layout moves it.
const STATE_AT: u32 = {
    let mut end = PAGE_BYTES + PARTS as u32 * spec::GAP_PAGES * PAGE_BYTES;
    let mut p = 0;
    while p < PARTS {
        end += page_up(PART_BYTES[p]);
        p += 1;
    }
    end
};

/// Only as wasm; a native build allocates from the process.
const HEAP_AT: u32 = page_up(STATE_AT + core::mem::size_of::<Guest>() as u32);

const _: () = assert!(
    HEAP_AT as usize + spec::HEAP_BYTES as usize <= spec::MEM_BYTES as usize,
    "the ranges run past the memory"
);
const _: () = assert!(
    (STATE_AT as usize).is_multiple_of(core::mem::align_of::<Guest>()),
    "the state lies on its own alignment"
);

pub type Now = extern "C" fn() -> f64;
pub type Hostcall = extern "C" fn(u32) -> i32;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    /// The native host's hostcall, which `boot` installs as the guest's.
    static HOSTCALL: Cell<Option<Hostcall>> = const { Cell::new(None) };
}

#[cfg(not(target_arch = "wasm32"))]
fn hostcall(at: u32) -> i32 {
    HOSTCALL
        .with(Cell::get)
        .map_or(Errno::ENOSYS as i32, |f| f(at))
}

struct Guest {
    door: Door,
    state: State,
}

struct State {
    stream: Stream,
    sys: Sys,
    files: Files<u32, { spec::HANDLES as usize }>,
    handles: Table<(), { spec::HANDLES as usize }>,
    now: Now,
    /// The clock at the last MARK.
    mark: f64,
    /// What HOSTCALL's host writes.
    word: u32,
}

/// Where each part lies under `seed`, from the range's start: the parts in the
/// order the seed draws, each past the pages it draws to leave before it. The
/// first page is left out (the contract's rule 1).
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

/// Every offset written is the memory's, so `range` is added to each.
///
/// # Safety
/// The range is `spec::MEM_BYTES` inside the memory, which outlives the
/// guest, and nothing else writes it while a call runs.
unsafe fn boot(mem: Memory, range: u32, now: Now, seed: u32) -> u32 {
    let at = layout(seed).map(|off| range + off);
    // SAFETY: the caller's memory; the ranges are the layout's.
    let (sq, cq, stream, sys) = unsafe {
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
            Stream::new(mem, at[STREAM] as usize, spec::STREAM_WORDS as usize, 1),
            Sys::new(mem, at[SYS] as usize, spec::SYS_WORDS as usize),
        )
    };
    let door = Door::new(sq, cq, spec::MOST_RAISE);
    // SAFETY: as above.
    unsafe {
        write_directory(
            mem,
            at[DIRECTORY] as usize,
            directory_words(4),
            &[
                (key::SQ, at[SQ_DESC]),
                (key::CQ, at[CQ_DESC]),
                (spec::key::STREAM, at[STREAM]),
                (key::SYS, at[SYS]),
            ],
        )
    };
    let guest = Guest {
        door,
        state: State {
            stream,
            sys,
            files: Files::new(),
            handles: Table::new(),
            now,
            mark: 0.0,
            word: 0,
        },
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { mem.at::<Guest>((range + STATE_AT) as usize).write(guest) };
    at[DIRECTORY]
}

/// # Safety
/// `boot` laid a guest out there.
unsafe fn turn(mem: Memory, range: u32) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let Guest { door, state } = unsafe { &mut *mem.at::<Guest>((range + STATE_AT) as usize) };
    state.sys.rise();
    door.drain(|cmd, pkt| apply(mem, door, state, cmd, pkt));
    state.stream.publish();
    state.sys.fall();
    if door.pending() {
        0.0
    } else {
        NO_WAKE
    }
}

/// # Safety
/// `mem` is `spec::MEM_BYTES`, 8-aligned, outlives the guest, and nothing
/// else writes it while a call runs.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(
    mem: *mut u8,
    now: Now,
    host: Hostcall,
    seed: u32,
) -> u32 {
    HOSTCALL.with(|h| h.set(Some(host)));
    xpute_guest::hostcall::set(hostcall);
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(given(mem), 0, now, seed) }
}

/// # Safety
/// `mem` is a memory `xpute_conformance_boot` laid a guest out over.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_run(mem: *mut u8, _quota_ms: f64) -> f64 {
    // SAFETY: the caller's memory, booted.
    unsafe { turn(given(mem), 0) }
}

/// The memory a native host hands over, `spec::MEM_BYTES` from `mem`.
#[cfg(not(target_arch = "wasm32"))]
fn given(mem: *mut u8) -> Memory {
    // SAFETY: the host hands over `spec::MEM_BYTES` from `mem`, kept for the guest's life.
    unsafe { Memory::new(mem.expose_provenance(), spec::MEM_BYTES as usize) }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use xpute_guest::entry::BootInfo;
    use xpute_guest::wasm::Config;

    // The heap is the range's last part, from `HEAP_AT` to the memory's end.
    xpute_guest::wasm_abi!(
        Config {
            heap_at: |data_end| range_at(data_end) + HEAP_AT,
            stack_bytes: spec::STACK_BYTES,
        },
        entry
    );

    extern "C" fn now() -> f64 {
        xpute_guest::wasm::now()
    }

    /// The range starts past where the linker stopped.
    fn range_at(data_end: u32) -> u32 {
        page_up(data_end)
    }

    #[no_mangle]
    pub extern "C" fn xpute_conformance_boot(seed: u32) -> u32 {
        let range = range_at(xpute_guest::wasm::data_end());
        // A host that sized the memory for another guest.
        xpute_kit::ensure!(
            range as u64 + spec::MEM_BYTES as u64 <= xpute_guest::wasm::given(),
            ENOMEM,
            range
        );
        // SAFETY: the module's memory, from address 0, and the range inside
        // it past what the linker placed.
        unsafe { boot(xpute_guest::mem::core(), range, now, seed) }
    }

    fn entry(_quota_ms: f64, _boot: fn() -> BootInfo) -> f64 {
        // SAFETY: the module's memory, booted.
        unsafe {
            turn(
                xpute_guest::mem::core(),
                range_at(xpute_guest::wasm::data_end()),
            )
        }
    }
}

fn apply(mem: Memory, door: &Door, state: &mut State, cmd: u32, pkt: &Option<&[u8]>) -> Applied {
    let State {
        stream,
        sys,
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
                door.post(k, spec::cmd::EVENT, 0, 0);
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
        spec::cmd::OPEN => Ok(Some(files.open(sys, 1, "f", arg(pkt)?) as i32)),
        spec::cmd::CLOSE => files
            .close(sys, arg(pkt)?)
            .map(|_| Some(0))
            .ok_or_else(bad_handle),
        spec::cmd::HOSTCALL => {
            let n = arg(pkt)?;
            *word = 0;
            let at = mem.off_of(&raw const *word) as u32;
            sys.hostcall(spec::hostcall::ANSWER, &[n, at], &[])
                .map_err(MarshalError::new)?;
            // SAFETY: the word the host wrote before the hostcall returned.
            Ok(Some(unsafe { (&raw const *word).read_volatile() } as i32))
        }
        c if c == Command::SYS_OPENED as u32 => {
            let answer = SysOpened::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?;
            if let Some(&owner) = files.owner(answer.fd) {
                door.post(owner, spec::cmd::OPENED, 0, answer.res);
            }
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}

fn arg(pkt: &Option<&[u8]>) -> Result<u32, MarshalError> {
    let bytes = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    Ok(TreeReader::new(bytes)?.read_branch()?.at(0)?.get_number()? as u32)
}
