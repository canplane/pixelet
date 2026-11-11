// xpute-conformance/core.rs

//! The Rust conformance core exports the same two calls as the C++ conformance
//! core, so that one runtime drives both. `boot` takes a seed, lays out the
//! parts of the front by it, writes the front's header as a runtime does from a
//! core's manifest, and answers the header's offset. Natively, the runtime hands
//! the core a region, a clock and its host (xpute-core host.rs). As wasm, the
//! core imports them, its range starts past the linker's data, and its turn is
//! the module's `run`.

#[cfg(not(target_arch = "wasm32"))]
use core::cell::Cell;

use xpute_core::abi::fs::{completion, Completion};
use xpute_core::abi::handle::Table;
use xpute_core::io::door::{self, door, Door, Event, Import, Outcome, Reply, ABORT_PKT_BYTES};
use xpute_core::io::file::FileTable;
use xpute_core::io::frame::WORD_BYTES;
use xpute_core::io::fs::Token;
use xpute_core::io::head::{self, Head};
use xpute_core::io::stream::Stream;
use xpute_core::mem::Memory;
use xpute_core::sched::wake::NO_WAKE;
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

// The parts the core shares with its runtime lie each on pages of its own, and
// `layout` places them in an order the seed draws.
const HEAD: usize = 0;
const RECEIVE: usize = 1;
const SEND: usize = 2;
const STREAM: usize = 3;
const PARTS: usize = 4;

/// A frame's room: its words and the most its packet takes.
const FRAME_BYTES: u32 = WORD_BYTES + spec::PKT_BYTES;

const PART_BYTES: [u32; PARTS] = [
    head::spec::WORDS * 4,
    FRAME_BYTES,
    FRAME_BYTES,
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
const HEAP_AT: u32 = page_up(STATE_AT + core::mem::size_of::<State>() as u32);

const _: () = assert!(spec::PKT_BYTES >= ABORT_PKT_BYTES);

/// Every OPEN names this file; no trace's runtime serves it.
const PATH: &[Token] = &[Token::Int(1), Token::Str("f")];

const _: () = assert!(
    HEAP_AT as usize + spec::HEAP_BYTES as usize <= spec::MEM_BYTES as usize,
    "the ranges run past the memory"
);
const _: () = assert!(
    (STATE_AT as usize).is_multiple_of(core::mem::align_of::<State>()),
    "the state lies on its own alignment"
);

pub type Now = extern "C" fn() -> f64;
pub type Receive = extern "C" fn(at: u32, cap: u32) -> i32;
pub type Send = extern "C" fn(at: u32, named_at: u32, named_len: u32);
pub type Call = extern "C" fn(at: u32) -> i32;

/// What the native runtime hands the core at boot: its region, its clock and
/// its host, which the core's first turn installs (`boot_info`).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct Native {
    mem: *mut u8,
    now: Now,
    receive: Receive,
    send: Send,
    call: Call,
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static NATIVE: Cell<Option<Native>> = const { Cell::new(None) };
}

#[cfg(not(target_arch = "wasm32"))]
fn native() -> Native {
    NATIVE.with(Cell::get).expect("a booted core")
}

struct State {
    stream: Stream,
    files: FileTable<u32, (), Vec<u8>, { spec::HANDLES as usize }>,
    handles: Table<(), { spec::HANDLES as usize }>,
    now: Now,
    /// This is the clock at the last MARK.
    mark: f64,
    /// What the runtime writes as it answers CALL's INC.
    word: u32,
    /// What NAMED's reply names, byte k holding k.
    named: [u8; spec::MOST_NAMED as usize],
}

/// CALL makes this import: a number, and the offset of the word the runtime
/// writes in answer.
struct Inc {
    n: u32,
    at: u32,
}

impl Import for Inc {
    const NUMBER: u32 = spec::cmd::INC;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.n));
        w.u32(Some(self.at));
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// `layout` answers where each part lies under `seed`, counted from the range's
/// start. The seed draws the order of the parts and, for each part, how many
/// pages to leave before it. The first page is never used, since offset 0 names
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
    // SAFETY: the caller's memory; the range is the layout's.
    let stream = unsafe {
        Stream::new(
            mem,
            at[STREAM] as usize,
            spec::STREAM_WORDS as usize,
            Stream::OWN_WORDS,
        )
    };
    door::set(Door::new(mem, at[RECEIVE], at[SEND], FRAME_BYTES));
    // The stream lies where a GPU stream would, so the runtime runs it after
    // every turn.
    let front = Head {
        pkt_bytes: spec::PKT_BYTES,
        stream_at: at[STREAM],
    };
    // SAFETY: as above.
    unsafe { head::write(mem, at[HEAD] as usize, &front) };
    let state = State {
        stream,
        files: FileTable::new(),
        handles: Table::new(),
        now,
        mark: 0.0,
        word: 0,
        named: [0; spec::MOST_NAMED as usize],
    };
    // SAFETY: the state's range is the layout's, inside the memory and on
    // the state's alignment (the asserts above).
    unsafe { mem.at::<State>((range + STATE_AT) as usize).write(state) };
    at[HEAD]
}

/// # Safety
/// `boot` must have laid out a core at `range` in `mem`.
unsafe fn turn(mem: Memory, range: u32) -> f64 {
    // SAFETY: the state boot made, which nothing else holds while a turn runs.
    let state = unsafe { &mut *mem.at::<State>((range + STATE_AT) as usize) };
    door().receive(|event| match event {
        Event::Completion(c) => {
            match completion(&c) {
                Some(Completion::FsOpen { fd, res, reply }) => {
                    let detail = reply.map_or(0, |r| r.detail);
                    if let Some((owner, _)) = state.files.complete_open(fd, res, detail) {
                        let code = res.map_or_else(|e| e as i32, |n| n as i32);
                        door().post(owner, spec::cmd::OPEN_COMPLETION, 0, code);
                    }
                }
                Some(Completion::FsClose { fd, res }) => {
                    state.files.complete_close(fd, res);
                }
                None => {}
            }
            Ok(Some(0.into()))
        }
        Event::Program(cmd, pkt) => apply(mem, door(), state, cmd, &pkt),
        // The conformance core keeps nothing across its life, reads no file
        // again, and draws nothing, so it answers 0 to every other
        // event.
        _ => Ok(Some(0.into())),
    });
    state.stream.publish();
    NO_WAKE
}

/// # Safety
/// `mem` must be `spec::MEM_BYTES` long, 8-aligned and outlive the core, and
/// nothing else may write it while a call runs.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_boot(
    mem: *mut u8,
    now: Now,
    receive: Receive,
    send: Send,
    call: Call,
    seed: u32,
) -> u32 {
    NATIVE.with(|h| {
        h.set(Some(Native {
            mem,
            now,
            receive,
            send,
            call,
        }))
    });
    // SAFETY: the caller's memory, the range from its start.
    unsafe { boot(given(mem), 0, now, seed) }
}

/// # Safety
/// `mem` must be the memory over which `xpute_conformance_boot` laid out a
/// core.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn xpute_conformance_run(_mem: *mut u8, deadline_ms: f64) -> f64 {
    xpute_core::entry::run(deadline_ms, boot_info, |_, _| {
        // SAFETY: the memory boot was handed, booted.
        unsafe { turn(given(native().mem), 0) }
    })
}

/// What the native runtime handed the core, as every core's entry takes it. The
/// core allocates from the process, so it reports no heap of its own.
#[cfg(not(target_arch = "wasm32"))]
fn boot_info() -> xpute_core::entry::BootInfo {
    xpute_core::entry::BootInfo {
        mem_sz: spec::MEM_BYTES,
        host: xpute_core::host::Host {
            receive: |at, cap| (native().receive)(at, cap),
            send: |at, named_at, named_len| (native().send)(at, named_at, named_len),
            call: |at| (native().call)(at),
        },
        heap_stat: Default::default,
        data_end: 0,
        now: || (native().now)(),
        stop_hook: None,
        publish: None,
    }
}

/// A native runtime hands over `spec::MEM_BYTES` from `mem`.
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
            range as u64 + spec::MEM_BYTES as u64 <= xpute_core::wasm::mem_sz(),
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

/// What each of the conformance protocol's commands answers.
fn apply(mem: Memory, door: &Door, state: &mut State, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
    let State {
        stream,
        files,
        handles,
        now,
        mark,
        word,
        named,
    } = state;
    let bad_handle = || MarshalError::new(Errno::EBADF);
    match cmd {
        spec::cmd::OK => Ok(Some(0.into())),
        spec::cmd::FAIL => Err(MarshalError::new(Errno::EINVAL)),
        spec::cmd::RAISE => {
            let n = arg(pkt)?;
            if n > spec::MOST_RAISE {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            for k in 0..n {
                door.post(k, spec::cmd::NOTE, 0, 0);
            }
            Ok(Some((n as i32).into()))
        }
        spec::cmd::RECORD => {
            stream.record(arg(pkt)?, &[], &[], None);
            Ok(Some(0.into()))
        }
        spec::cmd::ISSUE => Ok(Some(
            (handles
                .insert(())
                .ok_or_else(|| MarshalError::new(Errno::ENFILE))? as i32)
                .into(),
        )),
        spec::cmd::RELEASE => handles
            .remove(arg(pkt)?)
            .map(|()| Some(0.into()))
            .ok_or_else(bad_handle),
        spec::cmd::USE => handles
            .live(arg(pkt)?)
            .then_some(Some(0.into()))
            .ok_or_else(bad_handle),
        spec::cmd::MARK => {
            *mark = now();
            Ok(Some(0.into()))
        }
        spec::cmd::SINCE => Ok(Some((round(now() - *mark) as i32).into())),
        spec::cmd::OPEN => Ok(Some((files.open(PATH, arg(pkt)?) as i32).into())),
        spec::cmd::CLOSE => files
            .close(arg(pkt)?)
            .map(|()| Some(0.into()))
            .map_err(|_| bad_handle()),
        spec::cmd::CALL => {
            let n = arg(pkt)?;
            *word = 0;
            let at = mem.off_of(&raw const *word) as u32;
            door.call(&Inc { n, at }).map_err(MarshalError::new)?;
            // SAFETY: the word the runtime wrote before the call returned.
            Ok(Some(
                (unsafe { (&raw const *word).read_volatile() } as i32).into(),
            ))
        }
        spec::cmd::NAMED => {
            let n = arg(pkt)?;
            if n > spec::MOST_NAMED {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            for (k, b) in named[..n as usize].iter_mut().enumerate() {
                *b = k as u8;
            }
            // SAFETY: the core's state, which lies in its memory for the
            // core's life.
            let named = unsafe { core::slice::from_raw_parts(named.as_ptr(), n as usize) };
            Ok(Some(Reply { val: 0, named }))
        }
        _ => Ok(None),
    }
}

fn arg(pkt: &Option<&[u8]>) -> Result<u32, MarshalError> {
    let bytes = pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?;
    Ok(TreeReader::new(bytes)?.read_branch()?.at(0)?.get_number()? as u32)
}
