// pxl-kernel/profiler/summary.rs

//! The profiler window reduced into profiler.json's SUMMARY block, read in
//! place by the panel and the report so both agree. Sections go only through
//! `put`; indexing lanes by hand let sections drift out of order (2026-09-23).

use crate::billboard::sheet::billboard_sheet;
use crate::control::camera::controller;
use crate::io::failure::Outcome;
use crate::io::request::credits;
use crate::layer::Base as _;
use crate::profiler::block::CountersBlock;
use crate::profiler::block::TASK_PHASES;
use crate::profiler::block::{FetchBlock, FramesBlock, OpenBlock, QualityBlock, RecBlock, SheetBlock, Spread, StepBlock, StepsBlock, TaskBlock, TasksBlock, ViewBlock};
use crate::profiler::frame::{long, missed, off_turn_ms, overran};
use crate::profiler::frame::{FrameSample, GPU_SPANS};
use crate::profiler::log::window_baseline;
use crate::profiler::record::{fixed_lane, profiler_frames_since, profiler_open_interval, profiler_recording_state, profiler_tasks_since, Lane, SPREAD_LIMIT};
use crate::profiler::spec::{LIMITS, SUMMARY};
use crate::profiler::task::TaskRow;
use crate::profiler::word::put;
use crate::sched::tick::tick_step_stats;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::zoom::Zoom;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

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

/// No window is the whole recording.
pub fn profiler_summarize(window_ms: Option<f64>) -> &'static [f64] {
    let b = block();
    if b.scratch.is_empty() {
        b.scratch = fixed_lane(Lane::Scratch, SPREAD_LIMIT);
    }
    let (out, scratch) = (&mut b.words[..], &mut *b.scratch);
    out.fill(0.0);
    let since = window_ms.map_or(f64::NEG_INFINITY, |w| now() - w);

    let rec = profiler_recording_state();
    put(
        out,
        SUMMARY::rec::AT,
        &RecBlock {
            recording: rec.recording as u8 as f64,
            elapsed_ms: rec.elapsed_ms,
            truncated: rec.truncated as u8 as f64,
        },
    );

    put(out, SUMMARY::frames::AT, &frames(scratch, profiler_frames_since(since)));
    let open = profiler_open_interval().map_or(OpenBlock::default(), |f| OpenBlock { turns: f.turns, busy_ms: f.busy_ms });
    put(out, SUMMARY::open::AT, &open);
    put(out, SUMMARY::tasks::AT, &tasks(scratch, profiler_tasks_since(since)));
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
    let n = frames.len() as f64;

    let count = |of: &dyn Fn(&FrameSample) -> bool| frames.iter().filter(|f| of(f)).count() as f64;
    // A span the frame did not run reads 0 and is left out.
    // The GPU's figures over the frames a read landed with, each read once.
    let read = || frames.iter().filter(|f| f.gpu.read != 0.0);
    let spans: [Spread; GPU_SPANS] = core::array::from_fn(|k| spread(scratch, read().map(|f| f.gpu.spans[k]).filter(|&ms| ms > 0.0)));
    FramesBlock {
        present: 1.0,
        n,
        seconds,
        moving: moving().count() as f64,
        missed: moving().filter(|f| missed(f)).count() as f64,
        overran: count(&overran),
        long: count(&long),
        turns: sum(&|f| f.turns),
        interval: spread(scratch, moving().map(|f| f.interval_ms)),
        busy: spread(scratch, frames.iter().map(|f| f.busy_ms)),
        off_turn: spread(scratch, frames.iter().map(off_turn_ms)),
        tick: spread(scratch, frames.iter().map(|f| f.tick_ms)),
        tasks: spread(scratch, frames.iter().map(|f| f.tasks_ms)),
        gpu_read: count(&|f| f.gpu.read != 0.0),
        gpu: spread(scratch, read().map(|f| pxl_render::gpu::renderer::gpu_ms(&f.gpu.spans))),
        phases_p95: core::array::from_fn(|k| spread(scratch, frames.iter().map(|f| f.phases[k])).p95),
        gpu_spans_p95: spans.map(|s| s.p95),
        gpu_spans_max: spans.map(|s| s.max),
        kinds: core::array::from_fn(|k| count(&|f| f.kind == k as f64)),
        wakes: core::array::from_fn(|k| count(&|f| f.wake == k as f64)),
        heap_used_delta_bytes: last.heap.used_bytes - first.heap.used_bytes,
        census: core::array::from_fn(|k| [sum(&|f| f.gpu.census[k][0]) / n, sum(&|f| f.gpu.census[k][1]) / n]),
        last: *last,
    }
}

fn tasks(scratch: &mut [f64], tasks: &[TaskRow]) -> TasksBlock {
    let (ok, aborted, error) = (Outcome::ok as u32 as f64, Outcome::aborted as u32 as f64, Outcome::error as u32 as f64);
    let mut block = TasksBlock::default();
    for (c, slot) in block.slots.iter_mut().enumerate() {
        let mine = || tasks.iter().filter(move |r| r.stream == c as f64);
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

        let lat = spread(scratch, mine().filter(|r| r.outcome == ok).map(|r| r.end_ms - r.enqueue_ms));
        *slot = TaskBlock {
            n: n as f64,
            ok: mine().filter(|r| r.outcome == ok).count() as f64,
            aborted: mine().filter(|r| r.outcome == aborted).count() as f64,
            error: mine().filter(|r| r.outcome == error).count() as f64,
            lat_p50: lat.p50,
            lat_p95: lat.p95,
            reason_count: shown as f64,
            reasons: core::array::from_fn(|i| if i < shown { [counted[i].0, counted[i].1, counted[i].2] } else { [0.0; 3] }),
            phase_n: phase_n as f64,
            phase_mean: core::array::from_fn(|p| if seen[p] { sums[p] / phase_n as f64 } else { f64::NAN }),
        };
    }
    block
}

fn side(out: &mut [f64]) {
    put(out, SUMMARY::counters::AT, &counters().minus(&window_baseline()));

    let sheet = billboard_sheet().stats();
    put(
        out,
        SUMMARY::sheet::AT,
        &SheetBlock {
            size: sheet.size as f64,
            entries: sheet.entries as f64,
            shelves: sheet.shelves as f64,
            used_px: sheet.used_px as f64,
            initial: pxl_render::gpu::sheet::shelf::INITIAL_SIZE as f64,
        },
    );

    put(out, SUMMARY::fetch::AT, &FetchBlock { credits: credits() as f64 });

    put(
        out,
        SUMMARY::quality::AT,
        &QualityBlock {
            frames_held: TerrainTile::residency().frames_used() as f64,
        },
    );

    // What the pointer meets as drawn, whether or not it could strike it there.
    let met = crate::control::interaction::pointed().map(|at| (at, crate::view::lens::tau())).and_then(|([x, y], tau)| {
        let (mut origin, mut direction) = ([0.0; 3], [0.0; 3]);
        crate::view::frame::pose::view_ray(x, y, &mut origin, &mut direction);
        let ray = crate::query::Ray {
            origin,
            direction,
            reach: crate::view::observation::eye_reach().unwrap_or(0.0),
        };
        let hit = crate::query::World::now().first_hit(&ray, crate::query::Detail::Distance(tau)).value().flatten()?;
        // The grid's cell there is the distance's (`Tau::cell_at`), as the camera grid is cut.
        Some((crate::view::frame::coord::world_to_canonical(&hit.point), tau.cell_at(hit.t * hit.t)))
    });
    let (hover, cell_want) = (met.map(|m| m.0), met.map(|m| m.1));
    let [hover_x, hover_y, hover_z] = hover.map_or([f64::NAN; 3], |p| q3::unpack(&p));
    let [hover_lon, hover_lat, hover_alt] = hover.map_or([f64::NAN; 3], |p| epsg4326::unproject(&p));
    let [center_lon, center_lat, center_z] = controller::focus().map_or([f64::NAN; 3], |p| epsg4326::unproject(&p));
    let zoom = |z: Option<Zoom>| z.map_or(f64::NAN, f64::from);
    put(
        out,
        SUMMARY::view::AT,
        &ViewBlock {
            dist: controller::dist(),
            polar: controller::polar(),
            azimuth: controller::azimuth(),
            tau: crate::view::lens::tau().scale(),
            center_lon,
            center_lat,
            center_z,
            hover_x,
            hover_y,
            hover_z,
            hover_lon,
            hover_lat,
            hover_alt,
            cell_want: zoom(cell_want),
        },
    );

    // Per step: a cooperative scheduler is only as cooperative as its longest step.
    let mut steps = StepsBlock::default();
    for s in tick_step_stats().iter().filter(|s| s.count > 0).take(STEP_SLOTS) {
        steps.slots[steps.count as usize] = StepBlock {
            phase: s.phase as u8 as f64,
            name: s.name as f64,
            ms: s.ms,
            count: s.count as f64,
            max_ms: s.max_ms,
        };
        steps.count += 1.0;
    }
    put(out, SUMMARY::steps::AT, &steps);
}

/// The counters as they stand, from the billboard sheet and the terrain's
/// residency, which the window counts from its baseline.
pub fn counters() -> CountersBlock {
    let sheet = billboard_sheet().stats();
    let [admitted, evicted] = TerrainTile::residency().churn();
    CountersBlock {
        sheet_repacks: sheet.repacks as f64,
        sheet_uploads: sheet.uploads as f64,
        terrain_admitted: f64::from(admitted),
        terrain_evicted: f64::from(evicted),
    }
}
