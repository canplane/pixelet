// xpute-conformance/guest.rs

//! The Rust conformance guest: xpute-runtime's door and a stream over a
//! memory the host reserves, and guest.json's commands applied through them.
//! Where its ranges lie is its own choice; the directory it writes at
//! `spec::DIRECTORY_AT` is how the host finds them.
//!
//! Its two calls are C's, the same two the C++ guest exports
//! (cpp/xpute/conformance/guest.hpp), so one host drives every guest. A
//! guest is its memory: what it keeps lies in the range the host reserved,
//! so a call names the memory and two guests never share a thing.

use xpute_core::math::scalar::round;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::TreeReader;
use xpute_runtime::abi::handle::HandleTable;
use xpute_runtime::ipc::directory::{directory_words, spec::key, write_directory};
use xpute_runtime::ipc::doorbell::{Applied, Door, DoorLayout, RingLayout};
use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes};
use xpute_runtime::ipc::stream::Stream;
use xpute_runtime::sched::quantum::NO_WAKE;

use crate::spec;

const PAGE: u32 = 4096;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE) * PAGE
}

// The directory's page first, then each ring's descriptors, then their
// payloads, then the stream, then what the guest keeps.
const SUBMISSION_DESC_AT: u32 = PAGE;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(spec::SUBMISSION_CAPACITY));
const SUBMISSION_SLOT_AT: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(spec::COMPLETION_CAPACITY));
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(spec::SUBMISSION_CAPACITY, spec::SLOT_BYTES);
const STREAM_AT: u32 = COMPLETION_SLOT_AT + slots_bytes(spec::COMPLETION_CAPACITY, spec::SLOT_BYTES);
const STATE_AT: u32 = page_up(STREAM_AT + spec::STREAM_WORDS * 4);

const _: () = assert!(STATE_AT as usize + core::mem::size_of::<Guest>() <= spec::MEMORY_BYTES as usize, "the ranges run past the memory");
const _: () = assert!((STATE_AT as usize).is_multiple_of(core::mem::align_of::<Guest>()), "the state lies on its own alignment");
const _: () = assert!(directory_words(3) as u32 * 4 <= PAGE, "the directory outgrows its page");

/// The host's clock, as the host hands it over.
pub type Now = extern "C" fn() -> f64;

/// What the guest keeps, made in its own memory.
struct Guest {
    door: Door,
    stream: Stream,
    handles: HandleTable<{ spec::HANDLES as usize }>,
    now: Now,
    /// The clock at the last MARK.
    mark: f64,
}

/// Lays the guest out over `mem`, `spec::MEMORY_BYTES` of it and 8-aligned,
/// and writes its directory: what a guest's first turn does, here before it.
/// `now` is the host's clock.
///
/// # Safety
/// `mem` is that memory, outlives the guest, and nothing else writes it
/// while a call runs.
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(mem: *mut u8, now: Now) {
    let layout = DoorLayout {
        slot_bytes: spec::SLOT_BYTES,
        submission: RingLayout {
            capacity: spec::SUBMISSION_CAPACITY,
            desc_at: SUBMISSION_DESC_AT,
            slot_at: SUBMISSION_SLOT_AT,
        },
        completion: RingLayout {
            capacity: spec::COMPLETION_CAPACITY,
            desc_at: COMPLETION_DESC_AT,
            slot_at: COMPLETION_SLOT_AT,
        },
    };
    // SAFETY: the caller's memory; the ranges are the layout's.
    let (door, stream, directory) = unsafe {
        (
            Door::init(mem, &layout, |_, _| {}),
            Stream::new(core::slice::from_raw_parts_mut(mem.add(STREAM_AT as usize).cast::<u32>(), spec::STREAM_WORDS as usize), 1),
            core::slice::from_raw_parts_mut(mem.add(spec::DIRECTORY_AT as usize).cast::<u32>(), directory_words(3)),
        )
    };
    write_directory(
        directory,
        &[(key::SUBMISSION, SUBMISSION_DESC_AT), (key::COMPLETION, COMPLETION_DESC_AT), (spec::key::STREAM, STREAM_AT)],
    );
    let guest = Guest {
        door,
        stream,
        handles: HandleTable::new(),
        now,
        mark: 0.0,
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { mem.add(STATE_AT as usize).cast::<Guest>().write(guest) };
}

/// A turn of `quota_ms`, and when the guest wants the next: 0 while a
/// submission waits, and none otherwise.
///
/// # Safety
/// `mem` is a memory `xpute_conformance_boot` laid a guest out over.
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_interrupt(mem: *mut u8, _quota_ms: f64) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let Guest { door, stream, handles, now, mark } = unsafe { &mut *mem.add(STATE_AT as usize).cast::<Guest>() };
    door.drain(|cmd, pkt| apply(door, stream, handles, *now, mark, cmd, pkt));
    stream.publish();
    if door.pending() {
        0.0
    } else {
        NO_WAKE
    }
}

fn apply(door: &Door, stream: &mut Stream, handles: &mut HandleTable<{ spec::HANDLES as usize }>, now: Now, mark: &mut f64, cmd: u32, pkt: &Option<&[u8]>) -> Applied {
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
        _ => Ok(None),
    }
}

/// The number a command's packet carries.
fn arg(pkt: &Option<&[u8]>) -> Result<u32, MarshalError> {
    let bytes = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    Ok(TreeReader::new(bytes)?.read_branch()?.at(0)?.get_number()? as u32)
}
