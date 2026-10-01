// xpute-conformance/guest.rs

//! The Rust conformance guest: xpute-runtime's door and a stream over a
//! memory the host reserves, and guest.json's commands applied through them.
//! Where its ranges lie is its own choice, and `boot` answers where its
//! directory is — the one offset a host learns outside the memory (the
//! contract's rule 0).
//!
//! Its two calls are C's, the same two the C++ guest exports
//! (cpp/xpute/conformance/guest.hpp), so one host drives every guest, in two
//! forms by where the guest runs. Built as a native library, a guest is its
//! memory: the host hands it a region and a clock, and a call names the
//! region, so two guests never share a thing. Built as a WebAssembly module,
//! the memory is the one the module imports, begun at address 0, the clock is
//! its `env.now` import, and the range begins past what the linker placed.
//!
//! Every pointer here is made from an address, never by offsetting one: on
//! WebAssembly the memory's base is address 0, a null pointer, and `ptr::add`
//! from it is undefined (xpute-runtime mem/base.rs says what that cost once).

use xpute_core::math::scalar::round;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::TreeReader;
use xpute_runtime::abi::cmd::{Command, SysOpened};
use xpute_runtime::abi::handle::HandleTable;
use xpute_runtime::ipc::directory::{directory_words, spec::key, write_directory};
use xpute_runtime::ipc::doorbell::{Applied, Door, DoorLayout, RingLayout};
use xpute_runtime::ipc::file::Files;
use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes};
use xpute_runtime::ipc::stream::Stream;
use xpute_runtime::ipc::sys::Sys;
use xpute_runtime::sched::quantum::NO_WAKE;

use crate::spec;

const PAGE: u32 = 4096;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE) * PAGE
}

// Where this guest's ranges lie, from the start of its range: the
// directory's page first, then each ring's descriptors, then their payloads,
// then the stream, then the system calls' stream, then what the guest keeps,
// then its heap.
const DIRECTORY_AT: u32 = 0;
const SUBMISSION_DESC_AT: u32 = PAGE;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(spec::SUBMISSION_CAPACITY));
const SUBMISSION_SLOT_AT: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(spec::COMPLETION_CAPACITY));
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(spec::SUBMISSION_CAPACITY, spec::SLOT_BYTES);
const STREAM_AT: u32 = COMPLETION_SLOT_AT + slots_bytes(spec::COMPLETION_CAPACITY, spec::SLOT_BYTES);
const SYS_AT: u32 = STREAM_AT + spec::STREAM_WORDS * 4;
const STATE_AT: u32 = page_up(SYS_AT + spec::SYS_WORDS * 4);

/// The heap, past the state, on a page: a module's allocator over it, since
/// the memory never grows. A native build's is the process's.
const HEAP_AT: u32 = page_up(STATE_AT + core::mem::size_of::<Guest>() as u32);

const _: () = assert!(HEAP_AT as usize + spec::HEAP_BYTES as usize <= spec::MEMORY_BYTES as usize, "the ranges run past the memory");
const _: () = assert!((STATE_AT as usize).is_multiple_of(core::mem::align_of::<Guest>()), "the state lies on its own alignment");
const _: () = assert!(directory_words(4) as u32 * 4 <= PAGE, "the directory outgrows its page");

/// The host's clock, as the host hands it over.
pub type Now = extern "C" fn() -> f64;

/// What the guest keeps, made in its own memory: its door, and what the
/// commands it applies act on.
struct Guest {
    door: Door,
    state: State,
}

struct State {
    stream: Stream,
    sys: Sys,
    /// The files OPEN opened, each for the owner its packet named.
    files: Files<u32>,
    handles: HandleTable<{ spec::HANDLES as usize }>,
    now: Now,
    /// The clock at the last MARK.
    mark: f64,
}

/// A pointer `off` bytes into the memory whose base is `mem`, made from the
/// address.
fn at<T>(mem: *mut u8, off: u32) -> *mut T {
    core::ptr::with_exposed_provenance_mut(mem.expose_provenance() + off as usize)
}

/// Lays the guest out in the range that begins `range` bytes into the memory
/// at `mem`, and answers where its directory is. Every offset written
/// anywhere is the memory's, so the range's start is added to each.
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
            Door::init(mem, &layout, |_, _| {}),
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

/// A turn over the guest `boot` laid out at `range`, and when it wants the
/// next: 0 while a submission waits, and none otherwise.
///
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

/// Lays the guest out over `mem`, `spec::MEMORY_BYTES` of it and 8-aligned,
/// and writes its directory: what a guest's first turn does, here before it.
/// `now` is the host's clock. Answers the directory's offset.
///
/// # Safety
/// `mem` is that memory, outlives the guest, and nothing else writes it
/// while a call runs.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(mem: *mut u8, now: Now) -> u32 {
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(mem, 0, now) }
}

/// A turn of `quota_ms`, and when the guest wants the next.
///
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
    use xpute_core::alloc::buddy_tree::Range;
    use xpute_core::alloc::slab::SlabMalloc;

    /// The heap's range in the guest's own.
    struct Heap;

    const HEAP_LOG2: usize = spec::HEAP_BYTES.trailing_zeros() as usize;
    const SPLIT_BYTES: usize = (1 << (HEAP_LOG2 - 12)) / 8;

    impl Range for Heap {
        const MIN_LOG2: usize = 12;
        const MAX_LOG2: usize = HEAP_LOG2;
        type Split = [u8; SPLIT_BYTES];
        const SPLIT_INIT: Self::Split = [0; SPLIT_BYTES];
        fn base() -> usize {
            (range_at() + HEAP_AT) as usize
        }
        fn end() -> usize {
            Self::base() + spec::HEAP_BYTES as usize
        }
    }

    #[global_allocator]
    static ALLOC: SlabMalloc<Heap> = SlabMalloc::new();

    extern "C" {
        /// Where the linker stopped placing the module's stack and data.
        /// wasm-ld defines it.
        #[allow(non_upper_case_globals)]
        static __heap_base: u8;
    }

    #[link(wasm_import_module = "env")]
    extern "C" {
        #[link_name = "now"]
        fn host_now() -> f64;
    }

    extern "C" fn now() -> f64 {
        // SAFETY: an import with no arguments.
        unsafe { host_now() }
    }

    /// The range, from the first page past what the linker placed, so how
    /// large the stack or the data is decides nothing a host reads.
    fn range_at() -> u32 {
        page_up(core::ptr::addr_of!(__heap_base) as u32)
    }

    /// Lays the guest out past the module's own stack and data and writes
    /// its directory; answers the directory's offset.
    #[no_mangle]
    pub extern "C" fn xpute_conformance_boot() -> u32 {
        let range = range_at();
        let given = core::arch::wasm32::memory_size(0) as u64 * (1 << 16);
        // The host sized the memory; a range past its end is a host that
        // sized it for another guest.
        xpute_core::ensure!(range as u64 + spec::MEMORY_BYTES as u64 <= given, ENOMEM, range);
        // SAFETY: the module's memory, from address 0, and the range inside
        // it past what the linker placed.
        unsafe { boot(core::ptr::null_mut(), range, now) }
    }

    /// A turn of `quota_ms`, and when the guest wants the next.
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

/// The number a command's packet carries.
fn arg(pkt: &Option<&[u8]>) -> Result<u32, MarshalError> {
    let bytes = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    Ok(TreeReader::new(bytes)?.read_branch()?.at(0)?.get_number()? as u32)
}
