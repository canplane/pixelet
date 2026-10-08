// xpute-conformance/guest.rs

//! The Rust conformance guest. It exports the same two C calls as the C++
//! guest, so one host drives both; `boot` answers the directory's offset.
//! Natively the host hands it a region and a clock; as wasm it imports its
//! memory and `env.now`, and its range starts past what the linker placed.
//! Pointers are made from addresses: `ptr::add` from wasm's base 0 is UB.

use xpute_guest::abi::cmd::{Command, SysOpened};
use xpute_guest::abi::handle::HandleTable;
use xpute_guest::ipc::directory::{directory_words, spec::key, write_directory};
use xpute_guest::ipc::door::{Applied, Door, DoorLayout, RingLayout};
use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes};
use xpute_guest::ipc::stream::Stream;
use xpute_guest::ipc::sys::Sys;
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
const SUBMISSION_DESC_AT: u32 = PAGE_BYTES;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(spec::SUBMISSION_CAPACITY));
const SUBMISSION_SLOT_AT: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(spec::COMPLETION_CAPACITY));
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(spec::SUBMISSION_CAPACITY, spec::SLOT_BYTES);
const STREAM_AT: u32 = COMPLETION_SLOT_AT + slots_bytes(spec::COMPLETION_CAPACITY, spec::SLOT_BYTES);
const SYS_AT: u32 = STREAM_AT + spec::STREAM_WORDS * 4;
const STATE_AT: u32 = page_up(SYS_AT + spec::SYS_WORDS * 4);

/// Used only as wasm; a native build allocates from the process.
const HEAP_AT: u32 = page_up(STATE_AT + core::mem::size_of::<Guest>() as u32);

const _: () = assert!(HEAP_AT as usize + spec::HEAP_BYTES as usize <= spec::MEMORY_BYTES as usize, "the ranges run past the memory");
const _: () = assert!((STATE_AT as usize).is_multiple_of(core::mem::align_of::<Guest>()), "the state lies on its own alignment");
const _: () = assert!(directory_words(4) as u32 * 4 <= PAGE_BYTES, "the directory outgrows its page");

pub type Now = extern "C" fn() -> f64;

struct Guest {
    door: Door,
    state: State,
}

struct State {
    stream: Stream,
    sys: Sys,
    files: Files<u32>,
    handles: HandleTable<{ spec::HANDLES as usize }>,
    now: Now,
    /// The clock at the last MARK.
    mark: f64,
}

fn at<T>(mem: *mut u8, off: u32) -> *mut T {
    core::ptr::with_exposed_provenance_mut(mem.expose_provenance() + off as usize)
}

/// Every offset written is the memory's, so `range` is added to each.
///
/// # Safety
/// The range is `spec::MEMORY_BYTES` inside the memory, which outlives the
/// guest, and nothing else writes it while a call runs.
unsafe fn boot(mem: *mut u8, range: u32, now: Now) -> u32 {
    let layout = DoorLayout {
        slot_bytes: spec::SLOT_BYTES,
        submission: RingLayout {
            capacity: spec::SUBMISSION_CAPACITY,
            desc_at: range + SUBMISSION_DESC_AT,
            slot_at: range + SUBMISSION_SLOT_AT,
        },
        completion: RingLayout {
            capacity: spec::COMPLETION_CAPACITY,
            desc_at: range + COMPLETION_DESC_AT,
            slot_at: range + COMPLETION_SLOT_AT,
        },
    };
    // SAFETY: the caller's memory; the ranges are the layout's.
    let (door, stream, sys, directory) = unsafe {
        (
            Door::init(mem, &layout, spec::MOST_RAISE, |_, _| {}),
            Stream::new(core::slice::from_raw_parts_mut(at::<u32>(mem, range + STREAM_AT), spec::STREAM_WORDS as usize), 1),
            Sys::new(core::slice::from_raw_parts_mut(at::<u32>(mem, range + SYS_AT), spec::SYS_WORDS as usize)),
            core::slice::from_raw_parts_mut(at::<u32>(mem, range + DIRECTORY_AT), directory_words(4)),
        )
    };
    write_directory(
        directory,
        &[
            (key::SUBMISSION, range + SUBMISSION_DESC_AT),
            (key::COMPLETION, range + COMPLETION_DESC_AT),
            (spec::key::STREAM, range + STREAM_AT),
            (key::SYS, range + SYS_AT),
        ],
    );
    let guest = Guest {
        door,
        state: State {
            stream,
            sys,
            files: Files::new(),
            handles: HandleTable::new(),
            now,
            mark: 0.0,
        },
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { at::<Guest>(mem, range + STATE_AT).write(guest) };
    range + DIRECTORY_AT
}

/// # Safety
/// `boot` laid a guest out there.
unsafe fn interrupt(mem: *mut u8, range: u32) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let Guest { door, state } = unsafe { &mut *at::<Guest>(mem, range + STATE_AT) };
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
/// `mem` is `spec::MEMORY_BYTES`, 8-aligned, outlives the guest, and nothing
/// else writes it while a call runs.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(mem: *mut u8, now: Now) -> u32 {
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(mem, 0, now) }
}

/// # Safety
/// `mem` is a memory `xpute_conformance_boot` laid a guest out over.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_interrupt(mem: *mut u8, _quota_ms: f64) -> f64 {
    // SAFETY: the caller's memory, booted.
    unsafe { interrupt(mem, 0) }
}

#[cfg(target_arch = "wasm32")]
mod module {
    use super::*;
    use xpute_guest::mem::heap::{Bounds, Heap};
    use xpute_guest::module::{data_end, given};

    /// The heap is the range's last part, `HEAP_BYTES` from `HEAP_AT`.
    struct Range;

    impl Bounds for Range {
        fn base() -> usize {
            (range_at() + HEAP_AT) as usize
        }
        fn end() -> usize {
            Self::base() + spec::HEAP_BYTES as usize
        }
    }

    #[global_allocator]
    static ALLOC: Heap<Range> = Heap::new();

    extern "C" fn now() -> f64 {
        xpute_guest::module::now()
    }

    fn range_at() -> u32 {
        page_up(data_end())
    }

    #[no_mangle]
    pub extern "C" fn xpute_conformance_boot() -> u32 {
        let range = range_at();
        // A host that sized the memory for another guest.
        xpute_kit::ensure!(range as u64 + spec::MEMORY_BYTES as u64 <= given(), ENOMEM, range);
        // SAFETY: the module's memory, from address 0, and the range inside
        // it past what the linker placed.
        unsafe { boot(core::ptr::null_mut(), range, now) }
    }

    #[no_mangle]
    pub extern "C" fn xpute_conformance_interrupt(_quota_ms: f64) -> f64 {
        // SAFETY: the module's memory, booted.
        unsafe { interrupt(core::ptr::null_mut(), range_at()) }
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
                door.post(k, spec::cmd::SIGNAL, 0, 0, None);
            }
            Ok(Some(n as i32))
        }
        spec::cmd::RECORD => {
            stream.record(arg(pkt)?, &[], &[], None);
            Ok(Some(0))
        }
        spec::cmd::ISSUE => Ok(Some(handles.acquire().ok_or_else(|| MarshalError::new(Errno::ENFILE))? as i32)),
        spec::cmd::RELEASE => handles.release(arg(pkt)?).then_some(Some(0)).ok_or_else(bad_handle),
        spec::cmd::USE => handles.live(arg(pkt)?).then_some(Some(0)).ok_or_else(bad_handle),
        spec::cmd::MARK => {
            *mark = now();
            Ok(Some(0))
        }
        spec::cmd::SINCE => Ok(Some(round(now() - *mark) as i32)),
        spec::cmd::OPEN => Ok(Some(files.open(sys, 1, "f", arg(pkt)?) as i32)),
        spec::cmd::CLOSE => files.close(sys, arg(pkt)?).map(|_| Some(0)).ok_or_else(bad_handle),
        c if c == Command::SYS_OPENED as u32 => {
            let answer = SysOpened::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?;
            if let Some(&owner) = files.owner(answer.fd) {
                door.post(owner, spec::cmd::OPENED, 0, answer.res, None);
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
