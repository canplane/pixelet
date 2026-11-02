// pxl-kernel/door/apply/perf.rs

//! PERF: the profiler's.

use super::State;
use super::{ask_turn, given, refuse_unless};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::view::setting::store::settings_store;
use crate::perf::read;
use crate::perf::record;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::off_of;
use xpute_guest::mem::off_of_slice;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        // Records only while the profiler setting is on.
        Command::PERF_START => {
            refuse_unless(settings_store().perf, Errno::EPERM)?;
            refuse_unless(perf_window_start(), Errno::ENOMEM)?;
            ask_turn(s)
        }
        Command::PERF_STOP => {
            perf_window_stop();
            ask_turn(s)
        }
        // Memory is at most 1 GiB, so an address fits an i32.
        Command::PERF_STATS => {
            let window_ms = abi::PerfStats::read(given(pkt)?)?.window_ms;
            Ok(Some(read_timed(|| stats_words(window_ms)) as i32))
        }
        Command::PERF_RINGS => Ok(Some(read_timed(perf_rings) as i32)),
        _ => Ok(None),
    }
}

/// A read of the profiler, its time the profiler's cost to the interval
/// (perf/record.rs `perf_record_read`).
fn read_timed(read: impl FnOnce() -> u32) -> u32 {
    let at = now();
    let addr = read();
    record::perf_record_read(now() - at);
    addr
}

/// The address of STATS's words (perf.json): the statistics over
/// `window_ms`, and what holds now. Read in place, not posted: `Door::post`
/// asserts on a full completion ring. Valid until the next turn.
fn stats_words(window_ms: Option<f64>) -> u32 {
    use crate::perf::spec::STATS;
    // On the heap, not a static: a native build's statics are outside the shell-readable memory.
    static WORDS: Global<Box<[u32; STATS::WORDS as usize]>> = Global::new();
    let stats = read::perf_stats(window_ms);
    let now = read::perf_now();
    let w = WORDS.get(|| Box::new([0; STATS::WORDS as usize]));
    w[STATS::stats_at as usize] = off_of_slice(&raw const *stats) as u32;
    w[STATS::stats_words as usize] = stats.len() as u32;
    w[STATS::now_at as usize] = off_of_slice(&raw const *now) as u32;
    w[STATS::now_words as usize] = now.len() as u32;
    off_of(w.as_ptr()) as u32
}

/// The address of RINGS's words: the three rings as they lie.
fn perf_rings() -> u32 {
    use crate::perf::record::{perf_frames, perf_tasks, perf_worst};
    use crate::perf::spec::{RING, RINGS, RING_NAMES};
    // On the heap, as READ's words are.
    static WORDS: Global<Box<[u32; RINGS::WORDS as usize]>> = Global::new();
    let (frames, tasks, worst) = (perf_frames(), perf_tasks(), perf_worst());
    let w = WORDS.get(|| Box::new([0; RINGS::WORDS as usize]));
    // In RING_NAMES' order: frames, tasks, worst.
    let rings: [(usize, usize); RING_NAMES] = [
        (off_of_slice(&raw const *frames), frames.len()),
        (off_of_slice(&raw const *tasks), tasks.len()),
        (off_of_slice(&raw const *worst), worst.len()),
    ];
    for (k, (at, cnt)) in rings.into_iter().enumerate() {
        let slot = (RINGS::slots + k as u32 * RING::WORDS) as usize;
        w[slot + RING::at as usize] = at as u32;
        w[slot + RING::cnt as usize] = cnt as u32;
    }
    off_of(w.as_ptr()) as u32
}
