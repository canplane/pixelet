// pxl-kernel/sched/profiler/summary.rs

//! The profiler window reduced into profiler.json's SUMMARY block, read in
//! place by the panel and the report so both agree. Sections go only through
//! `put`; indexing lanes by hand let sections drift out of order (2026-09-23).

use crate::control::camera::controller;
use crate::io::sched::{counts, credits};
use crate::kernel::{kernel, with_world};
use crate::layer::billboard::sheet::billboard_sheet;
use crate::sched::profiler::block::TASK_PHASES;
use crate::sched::profiler::block::{AtlasBlock, FetchBlock, FramesBlock, HeapBlock, HeldBlock, QualityBlock, RecBlock, Spread, StepBlock, StepsBlock, TaskBlock, TasksBlock, ViewBlock};
use crate::sched::profiler::log::window_counters;
use crate::sched::profiler::record::{fixed_lane, profiler_frame_peak, profiler_frames_since, profiler_recording_state, profiler_tasks_since, put, FrameSample, Lane, Outcome, TaskRow, SPREAD_LIMIT};
use crate::sched::profiler::spec::{LIMITS, SUMMARY};
use crate::sched::tick::{tick_step_stats, HARD_SLACK_MS};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::adapter::epsg4326;
use pxl_spec::space::pvf3::projection;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

const MISSED_INTERVALS: f64 = 1.5;

/// A frame that dropped a vsync while moving. Still frames take a larger
/// budget on purpose, and a repeated image is no visible stutter.
pub fn missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.dt_ms > f.frame_ms * MISSED_INTERVALS
}

/// Past even the hard ceiling: one step too big for the forward-progress waiver.
pub fn overran(f: &FrameSample) -> bool {
    f.sched_ms > f.budget_ms + HARD_SLACK_MS
}

const WORDS: usize = SUMMARY::WORDS as usize;
const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;

struct Block {
    /// On the heap, not a static: a native build's statics are outside the shell-readable memory.
    words: Box<[f64; WORDS]>,
    scratch: &'static mut [f64],
}

static BLOCK: Global<Block> = Global::new();

fn block() -> &'static mut Block {
    BLOCK.get(|| Block {
        words: Box::new([0.0; WORDS]),
        scratch: &mut [],
    })
}

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

/// An infinite window is the whole recording.
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
        // A span the frame did not run reads 0 and is left out.
        gpu_spans: core::array::from_fn(|k| spread(scratch, frames.iter().map(|f| f.gpu.spans[k]).filter(|&ms| ms > 0.0))),
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
        zoom_tris: core::array::from_fn(|z| sum(&|f| f.gpu.zoom_tris[z]) / n),
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
            initial: pxl_render::asset::atlas::INITIAL_SIZE as f64,
        },
    );

    let held = with_world(|_, sched| sched.held()).unwrap_or_default();
    let (decor_blocks, decor_instances) = crate::layer::decor::cell::counts();
    put(
        out,
        SUMMARY::held::AT,
        &HeldBlock {
            run_sets: held.run_sets as f64,
            run_set_mb: held.run_set_mb as f64,
            tiers: held.tiers as f64,
            decor_blocks: decor_blocks as f64,
            decor_instances: decor_instances as f64,
            decor_bytes: crate::layer::decor::cell::bytes() as f64,
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
    let [refined, retiered, changes_peak] = kernel().sched.pace();
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
            retiered: retiered as f64,
            changes_peak: changes_peak as f64,
        },
    );

    let h = crate::host::heap_stat();
    put(
        out,
        SUMMARY::heap::AT,
        &HeapBlock {
            used_mb: h.used as f64 / (1 << 20) as f64,
            requests: h.requests as f64,
        },
    );

    let hover = crate::control::interaction::hover().coord;
    let [hover_x, hover_y, hover_z] = hover.map_or([f64::NAN; 3], |p| projection::unproject(p.i, p.f));
    let [hover_lon, hover_lat, hover_alt] = hover.map_or([f64::NAN; 3], |p| epsg4326::unproject(p.i, p.f));
    let [center_lon, center_lat, center_z] = controller::map_center().map_or([f64::NAN; 3], |p| epsg4326::unproject(p.i, p.f));
    let (chunk_zoom, tier_zoom, read_zoom) = hover
        .and_then(|p| with_world(|_, sched| sched.drawn_at(&p)).flatten())
        .map_or((None, None, None), |(c, t, r)| (Some(c), t, r));
    let zoom = |z: Option<i32>| z.map_or(f64::NAN, f64::from);
    put(
        out,
        SUMMARY::view::AT,
        &ViewBlock {
            dist: controller::dist(),
            polar: controller::polar(),
            azimuth: controller::azimuth(),
            tau: crate::world::pyramid::domain::current().map_or(f64::NAN, |d| d.tau().scale()),
            center_lon,
            center_lat,
            center_z,
            hover_x,
            hover_y,
            hover_z,
            hover_lon,
            hover_lat,
            hover_alt,
            chunk_zoom: zoom(chunk_zoom),
            tier_zoom: zoom(tier_zoom),
            read_zoom: zoom(read_zoom),
        },
    );

    // Per step: a cooperative scheduler is only as cooperative as its longest step.
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
