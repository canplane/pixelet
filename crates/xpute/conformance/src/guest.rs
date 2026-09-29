// xpute-conformance/guest.rs

//! The Rust conformance guest: xpute-runtime's door and a stream over a
//! memory the host reserves, and guest.json's commands applied through them.
//! Where its ranges lie is its own choice; the directory it writes at
//! `spec::DIRECTORY_AT` is how the host finds them.

use xpute_core::math::scalar::round;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::TreeReader;
use xpute_runtime::abi::handle::HandleTable;
use xpute_runtime::clock::now;
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
// payloads, then the stream.
const SUBMISSION_DESC_AT: u32 = PAGE;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(spec::SUBMISSION_CAPACITY));
const SUBMISSION_SLOT_AT: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(spec::COMPLETION_CAPACITY));
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(spec::SUBMISSION_CAPACITY, spec::SLOT_BYTES);
const STREAM_AT: u32 = COMPLETION_SLOT_AT + slots_bytes(spec::COMPLETION_CAPACITY, spec::SLOT_BYTES);

const _: () = assert!(STREAM_AT + spec::STREAM_WORDS * 4 <= spec::MEMORY_BYTES, "the ranges run past the memory");
const _: () = assert!(directory_words(3) as u32 * 4 <= PAGE, "the directory outgrows its page");

pub struct Guest {
    door: Door,
    stream: Stream,
    handles: HandleTable<{ spec::HANDLES as usize }>,
    /// The clock at the last MARK.
    mark: f64,
}

impl Guest {
    /// Lays the guest out over `mem` and writes its directory: what the
    /// first turn of a guest does, here before it.
    ///
    /// # Safety
    /// `mem` is `spec::MEMORY_BYTES`, 8-aligned, and outlives the guest,
    /// which is its only writer while it runs.
    pub unsafe fn boot(mem: *mut u8) -> Guest {
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
        Guest {
            door,
            stream,
            handles: HandleTable::new(),
            mark: 0.0,
        }
    }

    /// A turn: what waits applied, the stream's records ended, and the next
    /// turn asked for while any submission is left.
    pub fn interrupt(&mut self, _quota_ms: f64) -> f64 {
        let Guest { door, stream, handles, mark } = self;
        door.drain(|cmd, pkt| apply(door, stream, handles, mark, cmd, pkt));
        stream.publish();
        if door.pending() {
            0.0
        } else {
            NO_WAKE
        }
    }
}

fn apply(door: &Door, stream: &mut Stream, handles: &mut HandleTable<{ spec::HANDLES as usize }>, mark: &mut f64, cmd: u32, pkt: &Option<&[u8]>) -> Applied {
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
