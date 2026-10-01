// pxl-kernel/sched/profiler/summary.rs

//! The profiler window, reduced once, into a block the shell reads in place.
//!
//! The panel and the report both read this block, so a number on the panel and
//! the same number in a pasted report agree. It is laid out as profiler.json's
//! SUMMARY, every word an f64, written where it stays: a read builds nothing,
//! and the sorts a percentile needs run in a scratch lane taken once, so
//! reading the instrument does not allocate from the heap it measures.
//!
//! A section is built as its record (profiler/block.rs) and handed to `put`,
//! which is the only way anything reaches the block. Nothing here indexes a
//! lane, so no section can be written in an order of its own and no field can
//! be quietly left out — which is what this file used to allow, and did
//! (2026-09-23).

use crate::global::Global;
use crate::host::now;
use crate::io::sched::{counts, credits};
use crate::kernel::{kernel, with_world};
use crate::layer::billboard::sheet::billboard_sheet;
use crate::sched::profiler::block::{AtlasBlock, FetchBlock, FramesBlock, QualityBlock, RecBlock, RetainBlock, Spread, StepBlock, StepsBlock, TaskBlock, TasksBlock};
use crate::sched::profiler::log::window_counters;
use crate::sched::profiler::record::{fixed_lane, profiler_frame_peak, profiler_frames_since, profiler_recording_state, profiler_tasks_since, put, FrameSample, Lane, Outcome, TaskRow, SPREAD_LIMIT};
use crate::sched::profiler::spec::{LIMITS, SUMMARY, TASK_PHASE_NAMES};
use crate::sched::tick::{tick_step_stats, HARD_SLACK_MS};
use crate::world::chunk::build::RETAIN_BUDGET_BYTES;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

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
const TASK_PHASES: usize = TASK_PHASE_NAMES.len();
const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;

struct Block {
    /// On the heap rather than in the static: the shell reads it in the
    /// memory, and a native build's statics are not in it.
    words: Box<[f64; WORDS]>,
    /// Where a spread's values are gathered and sorted: as long as the longer
    /// ring, taken on the first read.
    scratch: &'static mut [f64],
}

static BLOCK: Global<Block> = Global::new();

fn block() -> &'static mut Block {
    BLOCK.get(|| Block {
        words: Box::new([0.0; WORDS]),
        scratch: &mut [],
    })
}

/// p50, p95 and max of `values`, zeros for none.
fn spread(scratch: &mut [f64], values: impl Iterator<Item = f64>) -> Spread {
    let mut n = 0;
    for v in values.take(scratch.len()) {
        scratch[n] = v;
        n += 1;
    }
    let sorted = &mut scratch[..n];
    sorted.sort_by(f64::total_cmp);
    let pct = |q: f64| if n == 0 { 0.0 } else { sorted[((n - 1) as f64 * q).floor() as usize] };
    Spread {
        p50: pct(0.5),
        p95: pct(0.95),
        max: if n == 0 { 0.0 } else { sorted[n - 1] },
    }
}

/// The window since `now - window_ms` reduced into the block, and the block —
/// an infinite window is the whole recording.
pub fn profiler_summarize(window_ms: f64) -> &'static [f64] {
    let b = block();
    if b.scratch.is_empty() {
        b.scratch = fixed_lane(Lane::Scratch, SPREAD_LIMIT);
    }
    let (out, scratch) = (&mut b.words[..], &mut *b.scratch);
    out.fill(0.0);
    let since = now() - window_ms;

    let rec = profiler_recording_state();
    put(
        out,
        SUMMARY::rec::AT,
        &RecBlock {
            recording: rec.recording as u8 as f64,
            elapsed_ms: rec.elapsed_ms,
            tasks: rec.tasks as f64,
            frames: rec.frames as f64,
            truncated: rec.truncated as u8 as f64,
        },
    );

    put(out, SUMMARY::frames::AT, &frames(scratch, profiler_frames_since(since)));
    put(out, SUMMARY::tasks::AT, &tasks(scratch, profiler_tasks_since(since)));
    put(out, SUMMARY::peak::AT, &profiler_frame_peak());
    side(out);
    out
}

fn frames(scratch: &mut [f64], frames: &[FrameSample]) -> FramesBlock {
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        return FramesBlock::default();
    };
    let sum = |pick: &dyn Fn(&FrameSample) -> f64| frames.iter().map(pick).sum::<f64>();
    let seconds = ((last.t - first.t) / 1000.0).max(1e-3);
    let moving = || frames.iter().filter(|f| f.settled == 0.0);
    let (scans, bakes) = (sum(&|f| f.work.scans), sum(&|f| f.work.bakes));
    let backlog = || frames.iter().filter(|f| f.lod.pending > 0.0);
    let backlog_s = (backlog().map(|f| f.dt_ms).sum::<f64>() / 1000.0).max(1e-3);
    let n = frames.len() as f64;

    FramesBlock {
        present: 1.0,
        n,
        seconds,
        moving: moving().count() as f64,
        missed: moving().filter(|f| missed(f)).count() as f64,
        overran: frames.iter().filter(|f| overran(f)).count() as f64,
        dt: spread(scratch, moving().map(|f| f.dt_ms)),
        task: spread(scratch, frames.iter().map(|f| f.task_ms)),
        offthread: spread(scratch, frames.iter().map(|f| (f.dt_ms - f.task_ms).max(0.0))),
        gpu: spread(scratch, frames.iter().map(|f| f.gpu.ms)),
        sched: spread(scratch, frames.iter().map(|f| f.sched_ms)),
        chunks: spread(scratch, frames.iter().map(|f| f.chunks_ms)),
        phases: core::array::from_fn(|k| spread(scratch, frames.iter().map(|f| f.phases[k]))),
        // A span the frame did not run reads 0 and is left out, so the air's
        // tables, drawn when the sun has moved, are what they cost when they
        // are.
        gpu_spans: core::array::from_fn(|k| spread(scratch, frames.iter().map(|f| f.gpu.spans[k]).filter(|&ms| ms > 0.0))),
        work_scans: scans,
        work_bakes: bakes,
        per_s_scans: scans / seconds,
        per_s_bakes: bakes / seconds,
        backlog_s,
        backlog_per_s_scans: backlog().map(|f| f.work.scans).sum::<f64>() / backlog_s,
        backlog_per_s_bakes: backlog().map(|f| f.work.bakes).sum::<f64>() / backlog_s,
        chunks_per_bake: if bakes > 0.0 { sum(&|f| f.chunks_ms) / bakes } else { f64::NAN },
        mounts_added: sum(&|f| f.mounts.added),
        mounts_removed: sum(&|f| f.mounts.removed),
        used_delta_mb: last.heap.used_mb - first.heap.used_mb,
        census: core::array::from_fn(|k| [sum(&|f| f.gpu.census[k][0]) / n, sum(&|f| f.gpu.census[k][1]) / n]),
        level_tris: core::array::from_fn(|k| sum(&|f| f.gpu.level_tris[k]) / n),
        last: *last,
    }
}

fn tasks(scratch: &mut [f64], tasks: &[TaskRow]) -> TasksBlock {
    let (ok, aborted, error) = (Outcome::ok as u32 as f64, Outcome::aborted as u32 as f64, Outcome::error as u32 as f64);
    let mut block = TasksBlock::default();
    for (c, slot) in block.slots.iter_mut().enumerate() {
        let mine = || tasks.iter().filter(move |r| r.category == c as f64);
        let n = mine().count();
        if n == 0 {
            continue;
        }

        // Failures by (reason, status), commonest first: a short list, counted
        // on the stack.
        let mut counted = [(0.0f64, 0.0f64, 0.0f64); 16];
        let mut kinds = 0;
        for r in mine().filter(|r| r.outcome == error) {
            match counted[..kinds].iter().position(|x| x.0 == r.fail && x.1 == r.detail) {
                Some(i) => counted[i].2 += 1.0,
                None if kinds < counted.len() => {
                    counted[kinds] = (r.fail, r.detail, 1.0);
                    kinds += 1;
                }
                None => {}
            }
        }
        counted[..kinds].sort_by(|x, y| y.2.total_cmp(&x.2));
        let shown = kinds.min(REASONS);

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

        *slot = TaskBlock {
            n: n as f64,
            ok: mine().filter(|r| r.outcome == ok).count() as f64,
            aborted: mine().filter(|r| r.outcome == aborted).count() as f64,
            error: mine().filter(|r| r.outcome == error).count() as f64,
            lat: spread(scratch, mine().filter(|r| r.outcome == ok).map(|r| r.end_ms - r.enqueue_ms)),
            reason_count: shown as f64,
            reasons: core::array::from_fn(|i| if i < shown { [counted[i].0, counted[i].1, counted[i].2] } else { [0.0; 3] }),
            phase_n: phase_n as f64,
            phase_mean: core::array::from_fn(|p| if seen[p] { sums[p] / phase_n as f64 } else { f64::NAN }),
        };
    }
    block
}

/// What the report prints beside the rings: the side counters, the badge
/// sheet, what chunks keep, the fetch queue, the POI placement and the steps.
fn side(out: &mut [f64]) {
    put(out, SUMMARY::counters::AT, &window_counters());

    let atlas = billboard_sheet().stats();
    put(
        out,
        SUMMARY::atlas::AT,
        &AtlasBlock {
            size: atlas.size as f64,
            entries: atlas.entries as f64,
            shelves: atlas.shelves as f64,
            used_px: atlas.used_px as f64,
        },
    );

    let retain = with_world(|_, sched| sched.retention()).unwrap_or_default();
    let census = kernel().sched.census;
    put(
        out,
        SUMMARY::retain::AT,
        &RetainBlock {
            run_sets: retain.run_sets as f64,
            run_set_mb: retain.run_set_mb as f64,
            tiers: retain.tiers as f64,
            decor_steps: retain.decor_steps as f64,
            decor_instances: retain.decor_instances as f64,
            on_bytes: census.retained_on,
            off_bytes: census.retained_off,
            budget_bytes: RETAIN_BUDGET_BYTES(),
        },
    );

    let (queued, running) = counts();
    put(
        out,
        SUMMARY::fetch::AT,
        &FetchBlock {
            queued: queued as f64,
            running: running as f64,
            credits: credits() as f64,
        },
    );

    let [step, star] = crate::world::quality::reading();
    let paging = Terrain::pyramid().paging();
    let [admitted, evicted] = paging.churn();
    let [refined, recubed, changes_peak] = kernel().sched.pace();
    put(
        out,
        SUMMARY::quality::AT,
        &QualityBlock {
            step: step as f64,
            star: star as f64,
            frames_held: paging.frames_used() as f64,
            admitted: admitted as f64,
            evicted: evicted as f64,
            refined: refined as f64,
            recubed: recubed as f64,
            changes_peak: changes_peak as f64,
        },
    );

    // Per step, so a phase that costs more than its steps should does not hide
    // which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    let mut steps = StepsBlock::default();
    for s in tick_step_stats().iter().filter(|s| s.count > 0).take(STEP_SLOTS) {
        steps.slots[steps.count as usize] = StepBlock {
            phase: s.phase as f64,
            name: s.name as f64,
            ms: s.ms,
            count: s.count as f64,
            max_ms: s.max_ms,
        };
        steps.count += 1.0;
    }
    put(out, SUMMARY::steps::AT, &steps);
}
