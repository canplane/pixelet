// pxl-core/door/apply/perf.rs

//! The PERF major's commands, which drive the profiler.

use super::State;
use super::{ask_turn, refuse_unless};
use crate::domain::setting::store::settings;
use crate::door::cmd as abi;
use crate::perf::read;
use crate::perf::record;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::io::door::Outcome;
use xpute_core::mem::off_of;
use xpute_core::mem::off_of_slice;
use xpute_kit::status::errno::Errno;

/// The profiler records only while its setting is on.
pub(super) fn start(s: &mut State) -> Outcome {
    refuse_unless(settings().perf, Errno::EPERM)?;
    refuse_unless(perf_window_start(), Errno::ENOMEM)?;
    ask_turn(s)
}

pub(super) fn stop(s: &mut State) -> Outcome {
    perf_window_stop();
    ask_turn(s)
}

/// Returns the address of the statistics' words. The core's memory is at
/// most 1 GiB, so an address fits in an i32.
pub(super) fn stats(c: abi::PerfStats) -> Outcome {
    Ok(Some(read_timed(|| stats_words(c.window_ms)) as i32))
}

/// Returns the address of the rings' words, which fits in an i32 for the
/// same reason.
pub(super) fn rings() -> Outcome {
    Ok(Some(read_timed(perf_rings) as i32))
}

/// Runs a read of the profiler and records its time as the profiler's own
/// cost to the interval (perf/record.rs `perf_record_read`).
fn read_timed(read: impl FnOnce() -> u32) -> u32 {
    let at = now();
    let addr = read();
    record::perf_record_read(now() - at);
    addr
}

/// The address of STAT_SLOT's words (perf.json): the statistics over
/// `window_ms` and the current values. The client reads them in place rather
/// than in a frame, whose packet takes no more than the manifest's
/// `pkt_bytes`.
/// The words stay valid until the next turn.
fn stats_words(window_ms: Option<f64>) -> u32 {
    use crate::perf::spec::STAT_SLOT;
    // The words live on the heap, not in a static, because a native build's
    // statics lie outside the core's memory, which is all the client reads.
    static WORDS: Global<Box<[u32; STAT_SLOT::WORDS as usize]>> = Global::new();
    let stats = read::perf_stats(window_ms);
    let now = read::perf_now();
    let w = WORDS.get(|| Box::new([0; STAT_SLOT::WORDS as usize]));
    w[STAT_SLOT::stats_at as usize] = off_of_slice(&raw const *stats) as u32;
    w[STAT_SLOT::stats_words as usize] = stats.len() as u32;
    w[STAT_SLOT::now_at as usize] = off_of_slice(&raw const *now) as u32;
    w[STAT_SLOT::now_words as usize] = now.len() as u32;
    off_of(w.as_ptr()) as u32
}

/// The address of RING_SLOT's words, which point at the three rings where they
/// lie.
fn perf_rings() -> u32 {
    use crate::perf::record::{perf_frames, perf_tasks, perf_worst};
    use crate::perf::spec::{RING, RING_NAMES, RING_SLOT};
    // On the heap for the same reason as STAT_SLOT's words (`stats_words`).
    static WORDS: Global<Box<[u32; RING_SLOT::WORDS as usize]>> = Global::new();
    let (frames, tasks, worst) = (perf_frames(), perf_tasks(), perf_worst());
    let w = WORDS.get(|| Box::new([0; RING_SLOT::WORDS as usize]));
    // In RING_NAMES' order: frames, tasks, worst.
    let rings: [(usize, usize); RING_NAMES] = [
        (off_of_slice(&raw const *frames), frames.len()),
        (off_of_slice(&raw const *tasks), tasks.len()),
        (off_of_slice(&raw const *worst), worst.len()),
    ];
    for (k, (at, cnt)) in rings.into_iter().enumerate() {
        let slot = (RING_SLOT::slots + k as u32 * RING::WORDS) as usize;
        w[slot + RING::at as usize] = at as u32;
        w[slot + RING::cnt as usize] = cnt as u32;
    }
    off_of(w.as_ptr()) as u32
}
