// pxl_core/module/world/sched/summary.rs

//! The ledger window, reduced once.
//!
//! The overlay and the report (log.rs) each used to reduce the rings
//! themselves, and they drifted: the report named failure reasons the panel
//! did not, and both counted missed frames against a 60 Hz constant the
//! display did not have. Both now format what this returns, so a number on
//! the panel and the same number in a pasted report agree.

use super::ledger::{DrawCensus, FrameSample, Outcome, TaskRecord, CENSUS_TAGS};
use super::tick::PHASES;
use super::tick::HARD_SLACK_MS;

/// Percentile over an already-sorted array; 0 for an empty one.
pub fn pct(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * q).floor().min((sorted.len() - 1) as f64) as usize]
}

/// One decoded terrain page, measured live (~4.03MB) — the multiplier on
/// tile_pages.
pub const TILE_DECODED_MB: f64 = 4.0;

/// A pending chunk waiting longer than this is being passed over rather than
/// queued. Ten seconds is above what a full zoom-out's queue takes to drain
/// (~7 s against one scan slot), so an ordinary backlog never trips it, and
/// below the sixteen-second wait that ageing exists to prevent: past it,
/// ageing is not doing its job.
pub const STARVED_MS: f64 = 10_000.0;

/// A gap this many display intervals long skipped a vsync.
const MISSED_INTERVALS: f64 = 1.5;

/// A frame that dropped a vsync while the view was moving. A still frame is
/// excluded: the tick takes a larger budget then on purpose (tick.rs), and a
/// repeated image is not a stutter anyone sees.
pub fn missed(f: &FrameSample) -> bool {
    !f.settled && f.dt_ms > f.frame_ms * MISSED_INTERVALS
}

/// The tick ran past what even the hard ceiling allows — a single step too big
/// for the forward-progress waiver to excuse.
pub fn overran(f: &FrameSample) -> bool {
    f.sched_ms > f.budget_ms + HARD_SLACK_MS
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

fn spread(mut values: Vec<f64>) -> Spread {
    values.sort_by(f64::total_cmp);
    Spread {
        p50: pct(&values, 0.5),
        p95: pct(&values, 0.95),
        max: values.last().copied().unwrap_or(0.0),
    }
}

/// [tag, draws, triangles], heaviest first.
pub type CensusRow = (&'static str, f64, f64);

#[derive(Clone, Debug, PartialEq)]
pub struct FrameSummary {
    pub n: usize,
    pub seconds: f64,
    /// Frames the view was moving in — the only ones pacing is judged on.
    pub moving: usize,
    pub missed: usize,
    pub overran: usize,
    pub dt: Spread,
    pub task: Spread,
    pub offthread: Spread,
    pub gpu: Spread,
    pub sched: Spread,
    pub tick: Spread,
    pub phases: Vec<(String, Spread)>,
    pub work: (f64, f64),
    /// Throughput over the whole window (scans, bakes per second).
    pub per_s: (f64, f64),
    /// Throughput while there was a backlog to drain. `per_s` divides by the
    /// whole window, idle stretches included, so two sessions of different
    /// length could not be compared on it; this can.
    pub backlog_s: f64,
    pub backlog_per_s: (f64, f64),
    /// What one bake cost the commit pass — comparable across sessions where
    /// the absolute commit time is not.
    pub tick_per_bake: Option<f64>,
    pub mounts: (f64, f64),
    /// Share of the window's pool acquires served from a region already
    /// reserved; None when the window acquired nothing.
    pub pool_hit_pct: Option<f64>,
    /// Per-frame mean draws and triangles by tag, heaviest first.
    pub census: Vec<CensusRow>,
    pub heap_delta: f64,
    pub last: FrameSample,
}

pub fn summarize_frames(frames: &[FrameSample]) -> Option<FrameSummary> {
    let (first, last) = (frames.first()?, frames.last()?);
    let of = |pick: &dyn Fn(&FrameSample) -> f64, from: &[&FrameSample]| spread(from.iter().map(|f| pick(f)).collect());
    let all: Vec<&FrameSample> = frames.iter().collect();
    let sum = |pick: &dyn Fn(&FrameSample) -> f64| frames.iter().map(pick).sum::<f64>();
    let seconds = ((last.t - first.t) / 1000.0).max(1e-3);
    let moving: Vec<&FrameSample> = frames.iter().filter(|f| !f.settled).collect();
    let work = (sum(&|f| f.work.scans), sum(&|f| f.work.bakes));
    let backlog: Vec<&FrameSample> = frames.iter().filter(|f| f.lod.pending > 0.0).collect();
    let backlog_s = (backlog.iter().map(|f| f.dt_ms).sum::<f64>() / 1000.0).max(1e-3);
    let tick_ms = sum(&|f| f.cpu.tick_ms);
    let pool_hits = last.pool.hits - first.pool.hits;
    let pool_acquires = pool_hits + last.pool.misses - first.pool.misses;

    Some(FrameSummary {
        n: frames.len(),
        seconds,
        moving: moving.len(),
        missed: moving.iter().filter(|f| missed(f)).count(),
        overran: frames.iter().filter(|f| overran(f)).count(),
        dt: of(&|f| f.dt_ms, &moving),
        task: of(&|f| f.task_ms, &all),
        offthread: of(&|f| (f.dt_ms - f.task_ms).max(0.0), &all),
        gpu: of(&|f| f.gpu_ms, &all),
        sched: of(&|f| f.sched_ms, &all),
        tick: of(&|f| f.cpu.tick_ms, &all),
        phases: PHASES.iter().enumerate().map(|(k, name)| (name.to_string(), spread(frames.iter().map(|f| f.phases[k]).collect()))).collect(),
        work,
        per_s: (work.0 / seconds, work.1 / seconds),
        backlog_s,
        backlog_per_s: (backlog.iter().map(|f| f.work.scans).sum::<f64>() / backlog_s, backlog.iter().map(|f| f.work.bakes).sum::<f64>() / backlog_s),
        tick_per_bake: (work.1 > 0.0).then(|| tick_ms / work.1),
        mounts: (sum(&|f| f.mounts.added), sum(&|f| f.mounts.removed)),
        pool_hit_pct: (pool_acquires > 0.0).then(|| ((pool_hits / pool_acquires) * 100.0 + 0.5).floor()),
        census: census_mean(frames),
        heap_delta: last.heap_mb - first.heap_mb,
        last: *last,
    })
}

fn census_mean(frames: &[FrameSample]) -> Vec<CensusRow> {
    let mut sums: DrawCensus = Default::default();
    for f in frames {
        for (s, (draws, tris)) in sums.iter_mut().zip(&f.census) {
            s.0 += draws;
            s.1 += tris;
        }
    }
    let n = frames.len().max(1) as f64;
    for s in &mut sums {
        *s = (s.0 / n, s.1 / n);
    }
    census_rows(&sums)
}

/// One census as rows, heaviest first, the pipelines that drew nothing left
/// out — the last frame's, say.
pub fn census_rows(census: &DrawCensus) -> Vec<CensusRow> {
    let mut rows: Vec<CensusRow> = CENSUS_TAGS.iter().zip(census).filter(|(_, (d, _))| *d > 0.0).map(|(tag, (d, t))| (*tag, *d, *t)).collect();
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    rows
}

pub fn tris_text(t: f64) -> String {
    if t >= 1e6 {
        format!("{:.2}M", t / 1e6)
    } else {
        format!("{}k", (t / 1000.0 + 0.5).floor())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSummary {
    pub cat: String,
    pub n: usize,
    pub ok: usize,
    pub aborted: usize,
    pub error: usize,
    /// Latency of the tasks that succeeded, enqueue to settle.
    pub lat: Spread,
    /// Failures by reason, commonest first — fetch_scheduler.rs's error_reason.
    pub reasons: Vec<(String, usize)>,
    /// Mean of each phase the demand's run reported, when any did — the number
    /// that says whether the category's static bound label is true.
    pub phase_mean: Option<Vec<(String, f64)>>,
}

pub fn summarize_tasks(tasks: &[TaskRecord]) -> Vec<TaskSummary> {
    struct Group {
        cat: String,
        lat: Vec<f64>,
        ok: usize,
        aborted: usize,
        error: usize,
        reasons: Vec<(String, usize)>,
        phase_sum: Vec<(String, f64)>,
        phase_n: usize,
    }
    let mut by: Vec<Group> = Vec::new();
    for rec in tasks {
        let g = match by.iter().position(|g| g.cat == rec.category) {
            Some(p) => &mut by[p],
            None => {
                by.push(Group {
                    cat: rec.category.to_string(),
                    lat: Vec::new(),
                    ok: 0,
                    aborted: 0,
                    error: 0,
                    reasons: Vec::new(),
                    phase_sum: Vec::new(),
                    phase_n: 0,
                });
                by.last_mut().expect("just pushed")
            }
        };
        match rec.outcome {
            Outcome::Ok => {
                g.ok += 1;
                g.lat.push(rec.end_ms - rec.enqueue_ms);
            }
            Outcome::Aborted => g.aborted += 1,
            Outcome::Error => {
                g.error += 1;
                let reason = rec.error.as_deref().unwrap_or("unrecorded").to_string();
                match g.reasons.iter_mut().find(|(r, _)| *r == reason) {
                    Some(r) => r.1 += 1,
                    None => g.reasons.push((reason, 1)),
                }
            }
        }
        if let Some(phases) = &rec.phases {
            g.phase_n += 1;
            for (k, v) in phases.iter() {
                match g.phase_sum.iter_mut().find(|(p, _)| p == k) {
                    Some(s) => s.1 += v,
                    None => g.phase_sum.push((k.to_string(), v)),
                }
            }
        }
    }
    by.into_iter()
        .map(|mut g| {
            g.reasons.sort_by(|a, b| b.1.cmp(&a.1));
            let n = g.phase_n as f64;
            TaskSummary {
                n: g.ok + g.aborted + g.error,
                ok: g.ok,
                aborted: g.aborted,
                error: g.error,
                lat: spread(g.lat),
                reasons: g.reasons,
                phase_mean: (g.phase_n > 0).then(|| g.phase_sum.into_iter().map(|(k, v)| (k, v / n)).collect()),
                cat: g.cat,
            }
        })
        .collect()
}
