// pxl-kernel/profiler/read.rs

//! What PROFILER_READ reads: the profiler window reduced into profiler.json's
//! STAT, and what holds as it reads into its NOW, each read in place by the
//! panel and the report so both agree. Sections go only through `put`;
//! indexing lanes by hand let sections drift out of order (2026-09-23).

use crate::domain::control::camera::controller;
use crate::domain::query;
use crate::domain::query::Ray;
use crate::domain::query::Struck;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::profiler::frame::FrameSample;
use crate::profiler::frame::{is_missed, long, overran, unmeasured};
use crate::profiler::record::{
    profiler_frames_since, profiler_recording_state, profiler_scratch, profiler_tasks_since,
};
use crate::profiler::section::TASK_PHASES;
use crate::profiler::section::{
    FramesSection, Spread, StepStats, StepsSection, StreamStats, TasksSection, ViewSection,
};
use crate::profiler::spec::{LIMIT, NOW, STAT};
use crate::profiler::task::TaskRow;
use crate::profiler::word::put;
use crate::sys::io::failure::Outcome;
use crate::sys::sched::tick::tick_step_stats;
use pxl_render::gpu::renderer;
use pxl_render::gpu::span_spec::FRAME_KINDS;
use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::zoom::Zoom;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

const STATS_WORDS: usize = STAT::WORDS as usize;
const NOW_WORDS: usize = NOW::WORDS as usize;
const REASONS: usize = LIMIT::REASONS as usize;
const STEP_SLOTS: usize = LIMIT::STEP_SLOTS as usize;

struct Read {
    /// On the heap, not a static: a native build's statics are outside the shell-readable memory.
    stats: Box<[f64; STATS_WORDS]>,
    now: Box<[f64; NOW_WORDS]>,
}

static READ: Global<Read> = Global::new();

fn read() -> &'static mut Read {
    READ.get(|| Read {
        stats: Box::new([0.0; STATS_WORDS]),
        now: Box::new([0.0; NOW_WORDS]),
    })
}

fn spread(scratch: &mut [f64], vals: impl Iterator<Item = f64>) -> Spread {
    let mut n = 0;
    for v in vals.take(scratch.len()) {
        scratch[n] = v;
        n += 1;
    }
    let sorted = &mut scratch[..n];
    sorted.sort_by(f64::total_cmp);
    let pct = |q: f64| {
        if n == 0 {
            0.0
        } else {
            sorted[((n - 1) as f64 * q).floor() as usize]
        }
    };
    Spread {
        p50: pct(0.5),
        p95: pct(0.95),
        max: if n == 0 { 0.0 } else { sorted[n - 1] },
    }
}

/// The window's statistics; no window is the whole recording.
pub fn profiler_stats(window_ms: Option<f64>) -> &'static [f64] {
    let (out, scratch) = (&mut read().stats[..], profiler_scratch());
    out.fill(0.0);
    let since = window_ms.map_or(f64::NEG_INFINITY, |w| now() - w);
    put(
        out,
        STAT::frames::AT,
        &frames(scratch, profiler_frames_since(since)),
    );
    put(
        out,
        STAT::tasks::AT,
        &tasks(scratch, profiler_tasks_since(since)),
    );
    put(out, STAT::steps::AT, &steps());
    out
}

/// What holds as it reads, the window aside.
pub fn profiler_now() -> &'static [f64] {
    let out = &mut read().now[..];
    out.fill(0.0);
    put(out, NOW::rec::AT, &profiler_recording_state());
    put(out, NOW::view::AT, &view());
    out
}

fn frames(scratch: &mut [f64], frames: &[FrameSample]) -> FramesSection {
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        return FramesSection::default();
    };
    let sum = |pick: &dyn Fn(&FrameSample) -> f64| frames.iter().map(pick).sum::<f64>();
    let seconds = ((last.t - first.t) / 1000.0).max(1e-3);
    let moving = || frames.iter().filter(|f| f.settled == 0.0);
    let n = frames.len() as f64;

    let cnt = |of: &dyn Fn(&FrameSample) -> bool| frames.iter().filter(|f| of(f)).count() as f64;
    // A span the frame did not run reads 0 and is left out.
    // The GPU's figures over the frames a read timed, each read once.
    let read = || frames.iter().filter(|f| f.gpu.read != 0.0);
    FramesSection {
        present: 1.0,
        n,
        seconds,
        moving: moving().count() as f64,
        moving_ms: moving().map(|f| f.interval_ms).sum(),
        missed: moving().filter(|f| is_missed(f)).count() as f64,
        overran: cnt(&overran),
        long: cnt(&long),
        turns: sum(&|f| f.turns),
        interval: spread(scratch, moving().map(|f| f.interval_ms)),
        busy: spread(scratch, frames.iter().map(|f| f.busy_ms)),
        tick: spread(scratch, frames.iter().map(|f| f.tick_ms)),
        effects: spread(scratch, frames.iter().map(|f| f.effects_ms)),
        stage: spread(scratch, frames.iter().map(|f| f.stage_ms)),
        tasks: spread(scratch, frames.iter().map(|f| f.tasks_ms)),
        profiler: spread(scratch, frames.iter().map(|f| f.profiler_ms)),
        rest: spread(scratch, frames.iter().map(unmeasured)),
        effects_p95: core::array::from_fn(|k| {
            spread(scratch, frames.iter().map(|f| f.effects[k])).p95
        }),
        gpu_read: cnt(&|f| f.gpu.read != 0.0),
        gpu: spread(scratch, read().map(|f| renderer::gpu_ms(&f.gpu.spans))),
        gpu_busy: spread(scratch, read().map(|f| f.gpu.busy_ms)),
        gpu_busy_kinds: {
            let mut by = [0.0; 2 * FRAME_KINDS];
            for k in 0..FRAME_KINDS {
                let busy = read().filter(|f| f.kind == k as f64).map(|f| f.gpu.busy_ms);
                let s = spread(scratch, busy);
                (by[2 * k], by[2 * k + 1]) = (s.p50, s.p95);
            }
            by
        },
        phases_p95: core::array::from_fn(|k| {
            spread(scratch, frames.iter().map(|f| f.phases[k])).p95
        }),
        gpu_spans_p95: core::array::from_fn(|k| {
            spread(
                scratch,
                read().map(|f| f.gpu.spans[k]).filter(|&ms| ms > 0.0),
            )
            .p95
        }),
        kinds: core::array::from_fn(|k| cnt(&|f| f.kind == k as f64)),
        wakes: core::array::from_fn(|k| cnt(&|f| f.wake == k as f64)),
        heap_used_delta_bytes: last.heap.used_bytes - first.heap.used_bytes,
        first: *first,
        last: *last,
    }
}

fn tasks(scratch: &mut [f64], tasks: &[TaskRow]) -> TasksSection {
    let (ok, aborted, err) = (
        Outcome::ok as u32 as f64,
        Outcome::aborted as u32 as f64,
        Outcome::err as u32 as f64,
    );
    let mut section = TasksSection::default();
    for (c, slot) in section.slots.iter_mut().enumerate() {
        let mine = || tasks.iter().filter(move |r| r.stream == c as f64);
        let n = mine().count();
        if n == 0 {
            continue;
        }

        let mut counted = [(0.0f64, 0.0f64, 0.0f64); 16];
        let mut kinds = 0;
        for r in mine().filter(|r| r.outcome == err) {
            match counted[..kinds]
                .iter()
                .position(|x| x.0 == r.fail && x.1 == r.detail)
            {
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
        for r in mine().filter(|r| r.phase_cnt > 0.0) {
            phase_n += 1;
            for k in 0..r.phase_cnt as usize {
                let id = r.phase_ids[k];
                if id >= 0.0 {
                    sums[id as usize] += r.phase_vals[k];
                    seen[id as usize] = true;
                }
            }
        }

        let lat = spread(
            scratch,
            mine()
                .filter(|r| r.outcome == ok)
                .map(|r| r.end_ms - r.enqueue_ms),
        );
        *slot = StreamStats {
            n: n as f64,
            ok: mine().filter(|r| r.outcome == ok).count() as f64,
            aborted: mine().filter(|r| r.outcome == aborted).count() as f64,
            err: mine().filter(|r| r.outcome == err).count() as f64,
            lat_p50: lat.p50,
            lat_p95: lat.p95,
            reason_cnt: shown as f64,
            reasons: core::array::from_fn(|i| {
                if i < shown {
                    [counted[i].0, counted[i].1, counted[i].2]
                } else {
                    [0.0; LIMIT::REASON_WORDS as usize]
                }
            }),
            phase_n: phase_n as f64,
            phase_mean: core::array::from_fn(|p| {
                if seen[p] {
                    sums[p] / phase_n as f64
                } else {
                    f64::NAN
                }
            }),
        };
    }
    section
}

fn view() -> ViewSection {
    use crate::domain::control::interaction::{self, collision};
    let pointed = interaction::pointed();
    // What the pointer meets, exactly, at the zooms the eye asks, whatever
    // the tool could strike there, and the zoom the rays draw it at.
    let met = pointed.and_then(|[x, y]| {
        let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
        let pose = pose::view();
        pose::ray_of(pose.at(), x, y, &mut origin, &mut dir);
        let ray = Ray {
            origin,
            dir,
            reach: observation::eye_draw_reach().unwrap_or(0.0),
        };
        let detail = query::by_dist();
        let hit = query::World::now()
            .first_hit(&ray, detail)
            .val()
            .flatten()?;
        Some((
            coord::world_to_canonical(&hit.point),
            detail.at((hit.t * hit.t) as f32),
        ))
    });
    let (hover, cell) = (met.map(|m| m.0), met.map(|m| m.1));
    // What the tool held strikes there, as the hover picks it.
    let picked = pointed.and_then(|[x, y]| interaction::pick_at(x, y));
    let struck = picked.as_ref().map_or(f64::NAN, |p| match p.hit.struck {
        Struck::Ground => 0.0,
        Struck::Object(_) => 1.0,
        Struck::Objects => 2.0,
        Struck::Decor(_) => 3.0,
    });
    let tool = collision::PlacementTool::of(&interaction::holding());
    let place = interaction::hover().coord.filter(|_| tool.is_some());
    let [place_x, place_y, place_z] = place.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let [hover_x, hover_y, hover_z] = hover.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let eye = controller::eye_world().map(|e| coord::world_to_canonical(&e));
    let [eye_x, eye_y, eye_z] = eye.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let pivot = controller::pivot();
    let [pivot_x, pivot_y, pivot_z] = pivot.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let [pivot_lon, pivot_lat, pivot_alt] =
        pivot.map_or([f64::NAN; 3], |p| epsg4326::unproject(&p));
    let zoom = |z: Option<Zoom>| z.map_or(f64::NAN, f64::from);
    ViewSection {
        eye_x,
        eye_y,
        eye_z,
        dist: controller::dist(),
        polar: controller::polar(),
        azimuth: controller::azimuth(),
        pivot_x,
        pivot_y,
        pivot_z,
        pivot_lon,
        pivot_lat,
        pivot_alt,
        hover_x,
        hover_y,
        hover_z,
        cell: zoom(cell),
        struck,
        held: zoom(picked.as_ref().map(|p| p.held())),
        drawn: zoom(picked.as_ref().and_then(|p| p.drawn)),
        folded: picked
            .as_ref()
            .map_or(f64::NAN, |p| f64::from(u8::from(p.is_folded()))),
        place_x,
        place_y,
        place_z,
        place_zoom: zoom(tool.map(|t| t.zoom())),
        blocked: tool.zip(place).map_or(f64::NAN, |(t, p)| {
            f64::from(u8::from(collision::is_placement_blocked(t, &p)))
        }),
    }
}

/// Per step: a cooperative scheduler is only as cooperative as its longest step.
fn steps() -> StepsSection {
    let mut steps = StepsSection::default();
    for s in tick_step_stats()
        .iter()
        .filter(|s| s.cnt > 0)
        .take(STEP_SLOTS)
    {
        steps.slots[steps.cnt as usize] = StepStats {
            phase: f64::from(s.phase),
            name: s.name as f64,
            ms: s.ms,
            cnt: s.cnt as f64,
            max_ms: s.max_ms,
        };
        steps.cnt += 1.0;
    }
    steps
}
