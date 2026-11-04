// xpute-example/lib.rs

//! The smallest guest, read beside the host that drives it
//! (ts/example/sum.test.ts). It answers one command, `SUM`, with the sum of
//! its two numbers and the answer to `ASK`, which it calls the asking port
//! with, answered before the call returns.
//!
//! What it lays out is its own: past where the linker stopped, a page for
//! the toc, then the two rings and their slots, then the heap. The host
//! finds them through the toc, whose offset `boot` answers.

#![cfg(target_arch = "wasm32")]

use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::ipc::door::{Door, Event, Outcome, Submission};
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes, Ring};
use xpute_guest::ipc::toc::{spec::key, toc_words, write_toc};
use xpute_guest::mem::section::PAGE_BYTES;
use xpute_guest::sched::wake::NO_WAKE;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// The command, the program's own: a branch of two numbers, answered with
/// their sum and the host's.
pub const SUM: u32 = 0x1001;
/// The call: a number its port answers, which the guest adds.
pub const ASK: u32 = 0x1002;
/// The port that sends `SUM` and answers `ASK`, the host test's.
pub const ASKER: u32 = 1;

/// `ASK`, which carries no fields.
struct Ask;

impl Event for Ask {
    const NUMBER: u32 = ASK;
    const FIELDS: u32 = 0;
    fn write(&self, _: &mut PacketWriter) {}
}

const CAP: u32 = 8;
const SLOT_BYTES: u32 = 256;
const STACK_BYTES: u32 = 1 << 16;

// From the range's start.
const TOC_AT: u32 = 0;
const SQ_AT: u32 = PAGE_BYTES;
const CQ_AT: u32 = SQ_AT + ring_bytes(CAP);
const SQ_SLOTS_AT: u32 = CQ_AT + ring_bytes(CAP);
const CQ_SLOTS_AT: u32 = SQ_SLOTS_AT + slots_bytes(CAP, SLOT_BYTES);
const HEAP_AT: u32 = (CQ_SLOTS_AT + slots_bytes(CAP, SLOT_BYTES)).next_multiple_of(PAGE_BYTES);

/// One a command may leave beside its reply: none.
const MOST_EVENTS: u32 = 0;

static DOOR: Global<Option<Door>> = Global::new();

xpute_guest::wasm_abi!(
    xpute_guest::wasm::Config {
        heap_at: |data_end| range_at(data_end) + HEAP_AT,
        stack_bytes: STACK_BYTES,
    },
    entry
);

/// The range starts on the page past where the linker stopped.
fn range_at(data_end: u32) -> u32 {
    data_end.next_multiple_of(PAGE_BYTES)
}

/// Lays the range out and writes the toc, before the first turn:
/// answers the toc's offset, the one the host learns from outside.
#[no_mangle]
pub extern "C" fn boot() -> u32 {
    let mem = xpute_guest::mem::core();
    let range = range_at(xpute_guest::wasm::data_end());
    let at = |off: u32| range + off;
    // SAFETY: the module's memory, the range past what the linker placed and
    // short of the heap, laid out once.
    let (sq, cq) = unsafe {
        (
            Ring::init(mem, at(SQ_AT) as usize, CAP, at(SQ_SLOTS_AT), SLOT_BYTES),
            Ring::init(mem, at(CQ_AT) as usize, CAP, at(CQ_SLOTS_AT), SLOT_BYTES),
        )
    };
    // SAFETY: as above; the toc's page is its own.
    unsafe {
        write_toc(
            mem,
            at(TOC_AT) as usize,
            toc_words(2),
            &[(key::SQ, at(SQ_AT)), (key::CQ, at(CQ_AT))],
        )
    };
    DOOR.set(Some(Door::new(sq, cq, MOST_EVENTS)));
    at(TOC_AT)
}

/// A turn: what waits applied in order, then the next asked for while any is left.
fn entry(_quota_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    let Some(door) = DOOR.get(|| None).as_ref() else {
        return NO_WAKE;
    };
    door.rise();
    door.receive(|submission| match submission {
        // It opens no file.
        Submission::Opened(_) => Ok(None),
        Submission::Program(cmd, pkt) => apply(door, cmd, &pkt),
    });
    if door.pending() {
        0.0
    } else {
        NO_WAKE
    }
}

fn apply(door: &Door, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
    if cmd != SUM {
        return Ok(None);
    }
    let pkt = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    let branch = TreeReader::new(pkt)?.read_branch()?;
    let (a, b) = (branch.at(0)?.get_number()?, branch.at(1)?.get_number()?);
    let answer = door.call(ASKER, &Ask).map_err(MarshalError::new)?;
    Ok(Some((a + b) as i32 + answer as i32))
}
