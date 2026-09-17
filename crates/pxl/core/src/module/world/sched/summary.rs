// pxl_core/module/world/sched/summary.rs

//! The ledger window, reduced once, into a block the shell reads in place.
//!
//! The panel and the report both read this block, so a number on the panel and
//! the same number in a pasted report agree. It is laid out as ledger.json's
//! SUMMARY, every word an f64, written where it stays: a read builds nothing,
//! and the sorts a percentile needs run in a scratch lane taken once, so
//! reading the instrument does not allocate from the heap it measures.

use super::ledger::{fixed_lane, ledger_frame_peak, ledger_frames, ledger_frames_since, ledger_recording_state, ledger_tasks_since, FrameSample, TaskRow, CENSUS_LEN, FRAME_LIMIT, TASK_LIMIT};
use super::ledger_spec::{CATEGORY_NAMES, LIMITS, OUTCOME_NAMES, PHASE_NAMES, SPREAD, STEP, STEP_NAMES, SUMMARY, TASK_PHASE_NAMES, TASK_SUMMARY};
use super::log::window_counters;
use super::tick::{tick_step_stats, HARD_SLACK_MS};
use crate::module::world::billboard::frustum_budget::billboard_dropped;
use crate::module::world::chunk_node::{chunk_retention, RETAIN_BUDGET_BYTES};
use crate::module::world::poi::poi_badge::badge_atlas;
use crate::module::world::poi::poi_index::PoiIndexStats;
use crate::module::world::poi::poi_layer::PoiPlaceStats;
use crate::module::world::sched::fetch_scheduler::{fetch_counts, fetch_credits};
use crate::sys::global::{now, Global};
use crate::sys::kernel::kernel;

use super::ledger::name_id;

/// A gap this many display intervals long skipped a vsync.
const MISSED_INTERVALS: f64 = 1.5;

/// A frame that dropped a vsync while the view was moving. A still frame is
/// excluded: the tick takes a larger budget then on purpose (tick.rs), and a
/// repeated image is not a stutter anyone sees.
pub fn missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.dt_ms > f.frame_ms * MISSED_INTERVALS
}

/// The tick ran past what even the hard ceiling allows — a single step too big
/// for the forward-progress waiver to excuse.
pub fn overran(f: &FrameSample) -> bool {
    f.sched_ms > f.budget_ms + HARD_SLACK_MS
}

const WORDS: usize = SUMMARY::WORDS as usize;
const CATEGORIES: usize = CATEGORY_NAMES.len();
const TASK_PHASES: usize = TASK_PHASE_NAMES.len();
const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;
const _: () = assert!(SUMMARY::tasks::WORDS as usize == CATEGORIES * TASK_SUMMARY::WORDS as usize);

struct Block {
    words: [f64; WORDS],
    /// Where a spread's values are gathered and sorted: as long as the longer
    /// ring, taken on the first read.
    scratch: &'static mut [f64],
}

static BLOCK: Global<Block> = Global::new();

fn block() -> &'static mut Block {
    BLOCK.get(|| Block {
        words: [0.0; WORDS],
        scratch: &mut [],
    })
}

fn at(section: u32, field: u32) -> usize {
    (section + field) as usize
}

/// p50, p95 and max of `values` at `out[at..]`, zeros for none.
fn spread(out: &mut [f64], scratch: &mut [f64], at: usize, values: impl Iterator<Item = f64>) {
    let mut n = 0;
    for v in values.take(scratch.len()) {
        scratch[n] = v;
        n += 1;
    }
    let sorted = &mut scratch[..n];
    sorted.sort_by(f64::total_cmp);
    let pct = |q: f64| if n == 0 { 0.0 } else { sorted[((n - 1) as f64 * q).floor() as usize] };
    out[at + SPREAD::p50 as usize] = pct(0.5);
    out[at + SPREAD::p95 as usize] = pct(0.95);
    out[at + SPREAD::max as usize] = if n == 0 { 0.0 } else { sorted[n - 1] };
}

/// The window since `now - window_ms` reduced into the block, and the block —
/// an infinite window is the whole recording.
pub fn ledger_summarize(window_ms: f64) -> &'static [f64] {
    let b = block();
    if b.scratch.is_empty() {
        b.scratch = fixed_lane(FRAME_LIMIT.max(TASK_LIMIT));
    }
    let (out, scratch) = (&mut b.words, &mut *b.scratch);
    out.fill(0.0);
    let since = now() - window_ms;

    let rec = ledger_recording_state();
    let r = SUMMARY::rec::AT;
    out[at(r, SUMMARY::rec::recording)] = rec.recording as u8 as f64;
    out[at(r, SUMMARY::rec::elapsed_ms)] = rec.elapsed_ms;
    out[at(r, SUMMARY::rec::tasks)] = rec.tasks as f64;
    out[at(r, SUMMARY::rec::frames)] = rec.frames as f64;
    out[at(r, SUMMARY::rec::truncated)] = rec.truncated as u8 as f64;

    frames(out, scratch, ledger_frames_since(since));
    tasks(out, scratch, ledger_tasks_since(since));
    side(out);
    out
}

fn frames(out: &mut [f64], scratch: &mut [f64], frames: &[FrameSample]) {
    use SUMMARY::frames as F;
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else { return };
    let a = F::AT;
    let sum = |pick: &dyn Fn(&FrameSample) -> f64| frames.iter().map(pick).sum::<f64>();
    let seconds = ((last.t - first.t) / 1000.0).max(1e-3);
    let moving = || frames.iter().filter(|f| f.settled == 0.0);
    let work = (sum(&|f| f.work.scans), sum(&|f| f.work.bakes));
    let backlog = || frames.iter().filter(|f| f.lod.pending > 0.0);
    let backlog_s = (backlog().map(|f| f.dt_ms).sum::<f64>() / 1000.0).max(1e-3);
    let tick_ms = sum(&|f| f.cpu.tick_ms);

    out[at(a, F::present)] = 1.0;
    out[at(a, F::n)] = frames.len() as f64;
    out[at(a, F::seconds)] = seconds;
    out[at(a, F::moving)] = moving().count() as f64;
    out[at(a, F::missed)] = moving().filter(|f| missed(f)).count() as f64;
    out[at(a, F::overran)] = frames.iter().filter(|f| overran(f)).count() as f64;
    spread(out, scratch, at(a, F::dt), moving().map(|f| f.dt_ms));
    spread(out, scratch, at(a, F::task), frames.iter().map(|f| f.task_ms));
    spread(out, scratch, at(a, F::offthread), frames.iter().map(|f| (f.dt_ms - f.task_ms).max(0.0)));
    spread(out, scratch, at(a, F::gpu), frames.iter().map(|f| f.gpu_ms));
    spread(out, scratch, at(a, F::sched), frames.iter().map(|f| f.sched_ms));
    spread(out, scratch, at(a, F::tick), frames.iter().map(|f| f.cpu.tick_ms));
    for k in 0..PHASE_NAMES.len() {
        spread(out, scratch, at(a, F::phases) + k * SPREAD::WORDS as usize, frames.iter().map(|f| f.phases[k]));
    }
    out[at(a, F::work_scans)] = work.0;
    out[at(a, F::work_bakes)] = work.1;
    out[at(a, F::per_s_scans)] = work.0 / seconds;
    out[at(a, F::per_s_bakes)] = work.1 / seconds;
    out[at(a, F::backlog_s)] = backlog_s;
    out[at(a, F::backlog_per_s_scans)] = backlog().map(|f| f.work.scans).sum::<f64>() / backlog_s;
    out[at(a, F::backlog_per_s_bakes)] = backlog().map(|f| f.work.bakes).sum::<f64>() / backlog_s;
    out[at(a, F::tick_per_bake)] = if work.1 > 0.0 { tick_ms / work.1 } else { f64::NAN };
    out[at(a, F::mounts_added)] = sum(&|f| f.mounts.added);
    out[at(a, F::mounts_removed)] = sum(&|f| f.mounts.removed);
    out[at(a, F::used_delta_mb)] = last.pool.used_mb - first.pool.used_mb;
    let n = frames.len() as f64;
    for k in 0..CENSUS_LEN {
        out[at(a, F::census) + 2 * k] = sum(&|f| f.census[k][0]) / n;
        out[at(a, F::census) + 2 * k + 1] = sum(&|f| f.census[k][1]) / n;
    }
    // SAFETY: a FrameSample is FRAME::WORDS plain f64s laid end to end
    // (ledger.rs asserts the layout).
    let words = unsafe { core::slice::from_raw_parts(last as *const FrameSample as *const f64, core::mem::size_of::<FrameSample>() / 8) };
    out[at(a, F::last)..at(a, F::last) + words.len()].copy_from_slice(words);
}

fn tasks(out: &mut [f64], scratch: &mut [f64], tasks: &[TaskRow]) {
    use TASK_SUMMARY as T;
    let (ok, aborted, error) = (name_id(OUTCOME_NAMES, "ok"), name_id(OUTCOME_NAMES, "aborted"), name_id(OUTCOME_NAMES, "error"));
    for c in 0..CATEGORIES {
        let a = SUMMARY::tasks::AT + SUMMARY::tasks::slots + c as u32 * T::WORDS;
        let mine = || tasks.iter().filter(move |r| r.category == c as f64);
        let n = mine().count();
        if n == 0 {
            continue;
        }
        out[at(a, T::n)] = n as f64;
        out[at(a, T::ok)] = mine().filter(|r| r.outcome == ok).count() as f64;
        out[at(a, T::aborted)] = mine().filter(|r| r.outcome == aborted).count() as f64;
        out[at(a, T::error)] = mine().filter(|r| r.outcome == error).count() as f64;
        spread(out, scratch, at(a, T::lat), mine().filter(|r| r.outcome == ok).map(|r| r.end_ms - r.enqueue_ms));

        // Failures by (reason, status), commonest first: a short list, counted
        // on the stack.
        let mut reasons = [(0.0f64, 0.0f64, 0.0f64); 16];
        let mut kinds = 0;
        for r in mine().filter(|r| r.outcome == error) {
            match reasons[..kinds].iter().position(|x| x.0 == r.fail && x.1 == r.status) {
                Some(i) => reasons[i].2 += 1.0,
                None if kinds < reasons.len() => {
                    reasons[kinds] = (r.fail, r.status, 1.0);
                    kinds += 1;
                }
                None => {}
            }
        }
        reasons[..kinds].sort_by(|x, y| y.2.total_cmp(&x.2));
        let shown = kinds.min(REASONS);
        out[at(a, T::reason_count)] = shown as f64;
        for (i, (fail, status, count)) in reasons[..shown].iter().enumerate() {
            let w = at(a, T::reasons) + 3 * i;
            (out[w], out[w + 1], out[w + 2]) = (*fail, *status, *count);
        }

        let mut sums = [0.0f64; TASK_PHASES];
        let mut seen = [false; TASK_PHASES];
        let mut phase_n = 0;
        for r in mine().filter(|r| r.phase_count > 0.0) {
            phase_n += 1;
            for k in 0..r.phase_count as usize {
                let id = r.phase_ids[k];
                if id >= 0.0 {
                    sums[id as usize] += r.phase_values[k];
                    seen[id as usize] = true;
                }
            }
        }
        out[at(a, T::phase_n)] = phase_n as f64;
        for p in 0..TASK_PHASES {
            out[at(a, T::phase_mean) + p] = if seen[p] { sums[p] / phase_n as f64 } else { f64::NAN };
        }
    }
}

/// What the report prints beside the rings: the side counters, the badge
/// sheet, what chunks keep, the fetch queue, the POI placement, the window's
/// peak and the steps.
fn side(out: &mut [f64]) {
    let c = window_counters();
    let a = SUMMARY::counters::AT as usize;
    let counters = [
        c.faces_kept,
        c.faces_possible,
        c.runs_culled,
        c.runs_total,
        c.coarse_boxes,
        c.coarse_low_fill,
        c.bands_reused,
        c.bands_allocated,
        c.built_direct,
        c.built_scans,
        c.built_bakes,
        c.built_starved,
        c.decor_dropped,
        c.badge_metrics,
        c.badge_builds,
        c.badge_ms,
        c.place_runs,
        c.atlas_repacks,
        c.atlas_uploads,
        c.index_rebuilds,
    ];
    out[a..a + counters.len()].copy_from_slice(&counters);

    let atlas = badge_atlas().stats();
    let a = SUMMARY::atlas::AT as usize;
    out[a..a + 4].copy_from_slice(&[atlas.size as f64, atlas.entries as f64, atlas.shelves as f64, atlas.used_px as f64]);

    let retain = chunk_retention();
    let census = kernel().sched.census;
    let a = SUMMARY::retain::AT as usize;
    out[a..a + 9].copy_from_slice(&[
        retain.nodes as f64,
        retain.run_sets as f64,
        retain.run_set_mb as f64,
        retain.tiers as f64,
        retain.decor_steps as f64,
        retain.decor_instances as f64,
        census.retained_on,
        census.retained_off,
        RETAIN_BUDGET_BYTES(),
    ]);

    let counts = fetch_counts();
    let a = SUMMARY::fetch::AT as usize;
    out[a..a + 3].copy_from_slice(&[counts.queued as f64, counts.running as f64, fetch_credits() as f64]);

    // POI is off until its index lives with its pages (issue/BACKLOG.md), so
    // these read zero rather than being left out: the report keeps their place.
    let (place, index) = (PoiPlaceStats::default(), PoiIndexStats::default());
    let a = SUMMARY::poi::AT as usize;
    out[a..a + 23].copy_from_slice(&[
        0.0,
        billboard_dropped() as f64,
        place.ms,
        place.hits as f64,
        place.candidates as f64,
        place.kept as f64,
        place.absorbed as f64,
        place.biggest as f64,
        place.max_ms,
        place.max_hits as f64,
        place.wait_ms,
        place.frames as f64,
        place.max_wait_ms,
        place.max_frames as f64,
        place.sync_ms,
        place.max_sync_ms,
        index.indexed as f64,
        index.sweeping as f64,
        index.dropped as f64,
        index.wait_ms,
        index.frames as f64,
        index.max_wait_ms,
        index.max_frames as f64,
    ]);

    let a = SUMMARY::peak::AT as usize;
    if !ledger_frames().is_empty() {
        let p = ledger_frame_peak();
        out[a..a + 17].copy_from_slice(&[
            1.0,
            p.dt_ms,
            p.sched_ms,
            p.tick_ms,
            p.draw_calls,
            p.triangles,
            p.q_terrain,
            p.q_poi,
            p.chunks,
            p.in_frustum,
            p.pending,
            p.settling,
            p.blend_active,
            p.pressure,
            p.offscreen_pending,
            p.oldest_release_ms,
            p.oldest_visible_ms,
        ]);
    }

    // Per step, so a phase that costs more than its steps should does not hide
    // which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    let a = SUMMARY::steps::AT as usize;
    let mut n = 0;
    for s in tick_step_stats().iter().filter(|s| s.count > 0).take(STEP_SLOTS) {
        let w = a + SUMMARY::steps::slots as usize + n * STEP::WORDS as usize;
        out[w + STEP::phase as usize] = s.phase as f64;
        out[w + STEP::name as usize] = name_id(STEP_NAMES, s.name);
        out[w + STEP::ms as usize] = s.ms;
        out[w + STEP::count as usize] = s.count as f64;
        out[w + STEP::max_ms as usize] = s.max_ms;
        n += 1;
    }
    out[a + SUMMARY::steps::count as usize] = n as f64;
}
