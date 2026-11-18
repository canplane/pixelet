// pxl-core/perf/read.rs

//! What `perf.stats` reads. The profiler's window is reduced into perf.json5's
//! STAT, and the state at the moment of the read into its NOW, which the
//! reply hands over together (`perf_read`). A section is written only through
//! `put` (perf/word.rs).

use crate::domain::control::camera;
use crate::domain::control::camera::controller;
use crate::domain::query;
use crate::domain::query::HitKind;
use crate::domain::query::Ray;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::frame::refused_frames;
use crate::perf::frame::{FrameSample, GPU_SPAN_CNT, RaySample};
use crate::perf::record::{
    perf_frames_since, perf_recording_extremes, perf_recording_state, perf_scratch,
    perf_tasks_since, step_stats,
};
use crate::perf::section::TASK_PHASE_CNT;
use crate::perf::section::{
    DrawSection, FrameSection, Percentile, StepSection, StepStat, StreamStat, TaskSection,
    ViewSection,
};
use crate::perf::task::TaskRecord;
use crate::perf::word::put;
use crate::spec;
use crate::spec::perf::{limit, now, stat};
use crate::sys::io::failure::Outcome;
use pxl_global::Global;
use pxl_render::gpu::api;
use pxl_render::renderer;
use pxl_render::spec::span::FrameKind;
use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::zoom::Zoom;
use xpute_guest::clock::now;

const REASON_CAP: usize = limit::REASON_CAP as usize;

/// STAT's words then NOW's, as `perf.stats`'s reply names them. They live on
/// the heap, not in a static, because a native build's statics lie outside the
/// core's memory, which is all a frame names.
static READ: Global<Box<[f64; stat::WORDS + now::WORDS]>> = Global::new();

fn read() -> &'static mut [f64; stat::WORDS + now::WORDS] {
    READ.get(|| Box::new([0.0; stat::WORDS + now::WORDS]))
}

/// The percentiles of `vals` (perf.json5 PERCENTILE) and how many values
/// they cover. Only as many values as `scratch` holds are taken.
fn percentile_n(scratch: &mut [f64], vals: impl Iterator<Item = f64>) -> (Percentile, usize) {
    let mut n = 0;
    for v in vals.take(scratch.len()) {
        scratch[n] = v;
        n += 1;
    }
    let sorted = &mut scratch[..n];
    sorted.sort_by(f64::total_cmp);
    (
        Percentile {
            p50: rank(sorted, 0.5),
            p95: rank(sorted, 0.95),
            max: sorted.last().copied().unwrap_or(0.0),
        },
        n,
    )
}

fn percentile(scratch: &mut [f64], vals: impl Iterator<Item = f64>) -> Percentile {
    percentile_n(scratch, vals).0
}

/// The nearest-rank percentile `q` of `sorted`: the value at index
/// ceil(q × n) − 1, the smallest value with at most a share `1 − q` of the
/// values above it. It is 0 for an empty slice.
fn rank(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    let at = ((q * n as f64).ceil() as usize).clamp(1, n) - 1;
    sorted[at]
}

/// The statistics over the last `window_ms`, or over the whole recording when
/// no window is given, then the state at the moment of the read, apart from
/// any window: STAT's words then NOW's.
pub(crate) fn perf_read(window_ms: Option<f64>) -> &'static [f64] {
    let words = read();
    stats(&mut words[..stat::WORDS], window_ms);
    state(&mut words[stat::WORDS..]);
    words
}

fn stats(out: &mut [f64], window_ms: Option<f64>) {
    let scratch = perf_scratch();
    out.fill(0.0);
    let since = window_ms.map_or(f64::NEG_INFINITY, |w| now() - w);
    put(
        out,
        stat::frames::AT,
        &frames(scratch, perf_frames_since(since)),
    );
    put(
        out,
        stat::tasks::AT,
        &tasks(scratch, perf_tasks_since(since)),
    );
    put(out, stat::steps::AT, &steps());
    put(out, stat::recording::AT, &perf_recording_extremes());
}

fn state(out: &mut [f64]) {
    out.fill(0.0);
    put(out, now::recording_state::AT, &perf_recording_state());
    put(out, now::draw::AT, &draw());
    put(out, now::view::AT, &view());
}

fn draw() -> DrawSection {
    let refused = refused_frames();
    let block = api::device_block();
    DrawSection {
        frame_cnt: renderer::drawn_frame_cnt(),
        refused_admit: refused.admit,
        refused_stream: refused.stream,
        generation: f64::from(api::device_generation()),
        status: f64::from(block.status),
        err_cnt: f64::from(block.err_cnt),
        unfinished: f64::from(block.unfinished),
    }
}

fn frames(scratch: &mut [f64], frames: &[FrameSample]) -> FrameSection {
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        return FrameSection::default();
    };
    let seconds = ((last.t - first.t) / 1000.0).max(1e-3);
    let moving = || frames.iter().filter(|f| f.settled == 0.0);
    let cnt = |of: &dyn Fn(&FrameSample) -> bool| frames.iter().filter(|f| of(f)).count() as f64;
    // The GPU's figures cover only the frames a read timed, each counted
    // once. A span the frame did not run reads 0 and is left out.
    let read = || frames.iter().filter(|f| f.gpu.timed != 0.0);
    let spans: [(f64, f64); GPU_SPAN_CNT] = core::array::from_fn(|k| {
        let ran = read().map(|f| f.gpu.spans[k]).filter(|&ms| ms > 0.0);
        let (s, n) = percentile_n(scratch, ran);
        (s.p95, n as f64)
    });
    FrameSection {
        present: 1.0,
        cnt: frames.len() as f64,
        seconds,
        moving: moving().count() as f64,
        moving_ms: moving().map(|f| f.interval_ms).sum(),
        overran: cnt(&|f| f.overran != 0.0),
        interval: percentile(scratch, moving().map(|f| f.interval_ms)),
        tick: percentile(scratch, frames.iter().map(|f| f.tick_ms)),
        effect: percentile(scratch, frames.iter().map(FrameSample::effects_ms)),
        pump: percentile(scratch, frames.iter().map(|f| f.pump_ms)),
        stage: percentile(scratch, frames.iter().map(FrameSample::stage_ms)),
        stage_parts_p95: core::array::from_fn(|k| {
            percentile(scratch, frames.iter().map(|f| f.stage_parts[k])).p95
        }),
        fill: percentile(scratch, frames.iter().map(|f| f.fill_ms)),
        fill_brick_cnt: frames.iter().map(|f| f.fill_brick_cnt).sum(),
        perf: percentile(scratch, frames.iter().map(|f| f.perf_ms)),
        effects_p95: core::array::from_fn(|k| {
            percentile(scratch, frames.iter().map(|f| f.effects[k])).p95
        }),
        invalidations: core::array::from_fn(|k| frames.iter().map(|f| f.invalidations[k]).sum()),
        gpu_timed: cnt(&|f| f.gpu.timed != 0.0),
        gpu_busy: percentile(scratch, read().map(|f| f.gpu.busy_ms)),
        gpu_busy_kinds: {
            let mut by = [0.0; 3 * FrameKind::ALL.len()];
            for k in 0..FrameKind::ALL.len() {
                let busy = read().filter(|f| f.kind == k as f64).map(|f| f.gpu.busy_ms);
                let (s, n) = percentile_n(scratch, busy);
                by[3 * k..3 * k + 3].copy_from_slice(&[s.p50, s.p95, n as f64]);
            }
            by
        },
        phases_p95: core::array::from_fn(|k| {
            percentile(scratch, frames.iter().map(|f| f.phases[k])).p95
        }),
        gpu_spans_p95: spans.map(|(p95, _)| p95),
        gpu_spans_cnt: spans.map(|(_, n)| n),
        kinds: core::array::from_fn(|k| cnt(&|f| f.kind == k as f64)),
        gpu_px: read().next_back().map_or(0.0, |f| f.gpu.px),
        ray_sample: frames.iter().fold(RaySample::default(), |mut sum, f| {
            sum.add(&f.ray_sample);
            sum
        }),
        heap_used_delta_bytes: last.heap_used_bytes - first.heap_used_bytes,
        last: *last,
    }
}

fn tasks(scratch: &mut [f64], tasks: &[TaskRecord]) -> TaskSection {
    let (ok, aborted, err) = (
        Outcome::Ok as u32 as f64,
        Outcome::Aborted as u32 as f64,
        Outcome::Err as u32 as f64,
    );
    let mut section = TaskSection::default();
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
        let shown = kinds.min(REASON_CAP);

        let mut sums = [0.0f64; TASK_PHASE_CNT];
        let mut seen = [false; TASK_PHASE_CNT];
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

        let lat = percentile(
            scratch,
            mine()
                .filter(|r| r.outcome == ok)
                .map(|r| r.end_ms - r.enqueue_ms),
        );
        *slot = StreamStat {
            cnt: n as f64,
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
                    [0.0; limit::REASON_WORDS as usize]
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
    use crate::domain::control::edit::{self, collision};
    let pointed = edit::pointer_ndc();
    // What the pointer's ray meets exactly, at the zooms the eye asks for,
    // whatever the tool could strike there, and the zoom the rays draw it at.
    let met = pointed.and_then(|[x, y]| {
        let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
        pose::ray_of(x, y, &mut origin, &mut dir);
        let ray = Ray {
            origin,
            dir,
            t_max: observation::eye_draw_dist().unwrap_or(0.0),
        };
        let hit = query::World::now().first_hit(&ray).val().flatten()?;
        Some((
            coord::world_to_canonical(&hit.point),
            query::cell_zoom_read(hit.point)?,
        ))
    });
    let (hover, cell_zoom) = (met.map(|m| m.0), met.map(|m| m.1));
    // What the held tool strikes there, picked the way the hover picks it.
    let picked = pointed.and_then(|[x, y]| edit::pick::pick_at(x, y));
    let hit_kind = picked.as_ref().map_or(f64::NAN, |p| {
        f64::from(match p.hit.kind {
            HitKind::Ground => spec::perf::HitKind::Ground,
            HitKind::Object(_) => spec::perf::HitKind::Object,
            HitKind::Objects => spec::perf::HitKind::Objects,
            HitKind::Decor(_) => spec::perf::HitKind::Decor,
        } as u8)
    });
    let asset = edit::tool().asset();
    let place = edit::hover().coord.filter(|_| asset.is_some());
    let [place_x, place_y, place_z] = place.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let [hover_x, hover_y, hover_z] = hover.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let eye = camera::eye_world().map(|e| coord::world_to_canonical(&e));
    let [eye_x, eye_y, eye_z] = eye.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let pivot = controller::pivot();
    let [pivot_x, pivot_y, pivot_z] = pivot.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let [pivot_lon, pivot_lat, pivot_ele] =
        pivot.map_or([f64::NAN; 3], |p| epsg4326::unproject(&p));
    let zoom = |z: Option<Zoom>| z.map_or(f64::NAN, f64::from);
    ViewSection {
        eye_x,
        eye_y,
        eye_z,
        dist: controller::dist(),
        polar: camera::polar(),
        azimuth: camera::azimuth(),
        pivot_x,
        pivot_y,
        pivot_z,
        pivot_lon,
        pivot_lat,
        pivot_ele,
        hover_x,
        hover_y,
        hover_z,
        cell_zoom: zoom(cell_zoom),
        hit_kind,
        pick_zoom: zoom(picked.as_ref().map(|p| p.zoom())),
        ray_zoom: zoom(picked.as_ref().and_then(|p| p.ray_zoom)),
        folded: picked
            .as_ref()
            .map_or(f64::NAN, |p| f64::from(u8::from(p.is_folded()))),
        place_x,
        place_y,
        place_z,
        place_zoom: zoom(asset.map(|(_, z)| z)),
        blocked: asset.zip(place).map_or(f64::NAN, |((a, z), p)| {
            f64::from(u8::from(collision::is_placement_blocked(a, z, &p)))
        }),
    }
}

fn steps() -> StepSection {
    let mut steps = StepSection::default();
    for (name, s) in step_stats() {
        steps.slots[steps.cnt as usize] = StepStat {
            name: f64::from(name as u8),
            ms: s.ms,
            cnt: s.cnt as f64,
            max_ms: s.max_ms,
        };
        steps.cnt += 1.0;
    }
    steps
}
