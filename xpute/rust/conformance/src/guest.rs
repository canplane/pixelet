// xpute-conformance/guest.rs

//! The Rust conformance guest, exporting the C++ guest's two calls so one
//! host drives both; `boot` answers the directory's offset. Natively the host
//! hands it a region and a clock; as wasm it imports both, its range starts
//! past the linker's data, and its turn is the module's `intr`.

use xpute_guest::abi::cmd::{Command, SysOpened};
use xpute_guest::abi::handle::Table;
use xpute_guest::ipc::directory::{directory_words, spec::key, write_directory};
use xpute_guest::ipc::door::{Applied, Door};
use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_guest::ipc::stream::Stream;
use xpute_guest::ipc::sys::Sys;
use xpute_guest::mem::Memory;
use xpute_guest::sched::quantum::NO_WAKE;
use xpute_kit::math::scalar::round;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::TreeReader;

use crate::spec;

use xpute_guest::mem::section::PAGE_BYTES;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

const DIRECTORY_AT: u32 = 0;
const SQ_DESC_AT: u32 = PAGE_BYTES;
const CQ_DESC_AT: u32 = SQ_DESC_AT + page_up(ring_bytes(spec::SQ_CAP));
const SQ_SLOT_AT: u32 = CQ_DESC_AT + page_up(ring_bytes(spec::CQ_CAP));
const CQ_SLOT_AT: u32 = SQ_SLOT_AT + slots_bytes(spec::SQ_CAP, spec::SLOT_BYTES);
const STREAM_AT: u32 = CQ_SLOT_AT + slots_bytes(spec::CQ_CAP, spec::SLOT_BYTES);
const SYS_AT: u32 = STREAM_AT + spec::STREAM_WORDS * 4;
const STATE_AT: u32 = page_up(SYS_AT + spec::SYS_WORDS * 4);

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
const _: () = assert!(
    directory_words(4) as u32 * 4 <= PAGE_BYTES,
    "the directory outgrows its page"
);

pub type Now = extern "C" fn() -> f64;

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
}

/// Every offset written is the memory's, so `range` is added to each.
///
/// # Safety
/// The range is `spec::MEM_BYTES` inside the memory, which outlives the
/// guest, and nothing else writes it while a call runs.
unsafe fn boot(mem: Memory, range: u32, now: Now) -> u32 {
    let at = |off: u32| (range + off) as usize;
    // SAFETY: the caller's memory; the ranges are the layout's.
    let (sq, cq, stream, sys) = unsafe {
        (
            Ring::init(
                mem,
                at(SQ_DESC_AT),
                spec::SQ_CAP,
                range + SQ_SLOT_AT,
                spec::SLOT_BYTES,
            ),
            Ring::init(
                mem,
                at(CQ_DESC_AT),
                spec::CQ_CAP,
                range + CQ_SLOT_AT,
                spec::SLOT_BYTES,
            ),
            Stream::new(mem, at(STREAM_AT), spec::STREAM_WORDS as usize, 1),
            Sys::new(mem, at(SYS_AT), spec::SYS_WORDS as usize),
        )
    };
    let door = Door::new(sq, cq, spec::MOST_RAISE);
    // SAFETY: as above.
    unsafe {
        write_directory(
            mem,
            at(DIRECTORY_AT),
            directory_words(4),
            &[
                (key::SQ, range + SQ_DESC_AT),
                (key::CQ, range + CQ_DESC_AT),
                (spec::key::STREAM, range + STREAM_AT),
                (key::SYS, range + SYS_AT),
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
        },
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { mem.at::<Guest>(at(STATE_AT)).write(guest) };
    range + DIRECTORY_AT
}

/// # Safety
/// `boot` laid a guest out there.
unsafe fn turn(mem: Memory, range: u32) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let Guest { door, state } = unsafe { &mut *mem.at::<Guest>((range + STATE_AT) as usize) };
    state.sys.rise();
    door.drain(|cmd, pkt| apply(door, state, cmd, pkt));
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
pub unsafe extern "C" fn xpute_conformance_boot(mem: *mut u8, now: Now) -> u32 {
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(given(mem), 0, now) }
}

/// # Safety
/// `mem` is a memory `xpute_conformance_boot` laid a guest out over.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_intr(mem: *mut u8, _quota_ms: f64) -> f64 {
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
    use xpute_guest::wasm::Plan;

    // The heap is the range's last part, from `HEAP_AT` to the memory's end.
    xpute_guest::wasm_abi!(
        Plan {
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
    pub extern "C" fn xpute_conformance_boot() -> u32 {
        let range = range_at(xpute_guest::wasm::data_end());
        // A host that sized the memory for another guest.
        xpute_kit::ensure!(
            range as u64 + spec::MEM_BYTES as u64 <= xpute_guest::wasm::given(),
            ENOMEM,
            range
        );
        // SAFETY: the module's memory, from address 0, and the range inside
        // it past what the linker placed.
        unsafe { boot(xpute_guest::mem::core(), range, now) }
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

fn apply(door: &Door, state: &mut State, cmd: u32, pkt: &Option<&[u8]>) -> Applied {
    let State {
        stream,
        sys,
        files,
        handles,
        now,
        mark,
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
                door.post(k, spec::cmd::SIGNAL, 0, 0);
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
