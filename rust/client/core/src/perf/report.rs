// pxl-core/perf/report.rs

//! Builds a frame's record once its frame is drawn, so every figure in
//! it belongs to that one frame: what the frame measured of itself, what the
//! renderer made of its frame, and the tiles each base layer took in.

use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use crate::sys::sched::tick::TickReport;
use pxl_global::Global;
use pxl_render::renderer::GpuRead;
use pxl_render::spec::span::FrameKind;
use pxl_spec::bake::publication::BaseKind;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::bake::tile::place::PlaceTile;
use xpute_guest::clock::now;

use crate::domain::layer::Base;
use crate::domain::layer::imagery::ImageryTile;
use crate::domain::scene::stage::FrameReport;
use crate::perf::frame::{
    DeviceSample, EFFECT_CNT, FrameSample, GpuSample, INVALIDATION_REASON_CNT, LAYER_CNT,
    RaySample, STAGE_PART_CNT,
};
use crate::perf::record::{
    RayRead, perf_fold_recording, perf_record_frame, perf_record_gpu, perf_record_rays,
    perf_recording,
};
use crate::perf::section::RecordingSection;
use crate::sys::heap::{self, HeapStat};
use crate::sys::io::body;

/// What the renderer made of this frame (domain/scene/stage.rs
/// `stage_render`), with the reads of earlier frames that landed this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RendererFrame {
    kind: FrameKind,
    gpu: GpuSample,
    device: DeviceSample,
    gpu_read: Option<GpuRead>,
    ray_read: Option<RayRead>,
}

impl Default for RendererFrame {
    fn default() -> Self {
        RendererFrame {
            kind: FrameKind::None,
            gpu: GpuSample::default(),
            device: DeviceSample::default(),
            gpu_read: None,
            ray_read: None,
        }
    }
}

static RENDERER: Global<RendererFrame> = Global::new();

/// Takes the renderer's report for this frame (domain/scene/stage.rs
/// `stage_render`) and the reads of earlier frames that landed this frame.
/// The figures are 0 when the renderer drew no frame.
pub(crate) fn set_renderer_frame(report: &FrameReport) {
    let (kind, scale) = (report.kind, report.scale);
    let Some(stat) = report.stat else {
        RENDERER.set(RendererFrame {
            kind,
            gpu: GpuSample {
                scale,
                ..GpuSample::default()
            },
            ..RendererFrame::default()
        });
        return;
    };
    let [draw_cnt, triangle_cnt] = stat
        .draw_stat
        .iter()
        .fold([0.0; 2], |[d, t], c| [d + c[0], t + c[1]]);
    RENDERER.set(RendererFrame {
        kind,
        gpu: GpuSample {
            scale,
            draw_cnt,
            triangle_cnt,
            frame: stat.frame,
            light_changed: stat.light_changed,
            ..GpuSample::default()
        },
        device: DeviceSample {
            upload_cnt: stat.upload_cnt,
            upload_bytes: stat.upload_bytes,
        },
        gpu_read: stat.gpu,
        ray_read: stat.ray_cnt.map(|r| RayRead {
            frame: r.frame,
            sample: RaySample::of(r.cnts),
        }),
    });
}

/// The tiles each base layer's tile cache took in and those it refused a
/// slot since the frame before, each indexed by `BaseKind`. Taking a count
/// starts it again from zero.
fn cache_cnts() -> ([f64; LAYER_CNT], [f64; LAYER_CNT]) {
    fn of<L: Base>() -> (f64, f64) {
        let cache = L::tile_cache();
        (
            f64::from(cache.take_load_cnt()),
            f64::from(cache.take_refusal_cnt()),
        )
    }
    let cnts = BaseKind::ALL.map(|layer| match layer {
        BaseKind::Terrain => of::<IndexedTerrainTile>(),
        BaseKind::Place => of::<PlaceTile>(),
        BaseKind::Division => of::<DivisionTile>(),
        BaseKind::Imagery => of::<ImageryTile>(),
    });
    (cnts.map(|c| c.0), cnts.map(|c| c.1))
}

/// What this frame held, for the recording's extremes: the heap as its
/// allocator reports it, the body slots' peak since the frame before, and
/// the tiles refused a slot since then, `refusals`. Taking a peak starts it
/// again from zero.
fn peaks(h: &HeapStat, refusals: [f64; LAYER_CNT]) -> RecordingSection {
    RecordingSection {
        frame_cnt: 1.0,
        heap_live_max_bytes: h.live as f64,
        heap_run_max_bytes: h.run as f64,
        heap_slot_max_bytes: h.slot as f64,
        heap_range_max_bytes: h.range as f64,
        heap_largest_free_min_bytes: h.largest_free as f64,
        heap_refusal_cnt: f64::from(h.refusals),
        body_slots_peak: f64::from(body::take_peak()),
        refusals,
    }
}

/// A frame's own measures (frame.rs `run`).
pub(crate) struct CpuSample {
    pub tick: TickReport,
    /// The gap since the last frame, which closes the interval before this
    /// one.
    pub gap_ms: f64,
    /// The display's interval as the frame's pacing takes it
    /// (domain/scene/rate.rs `display_ms`), and the frame's target interval,
    /// which it was paced at (`target_ms`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    /// Whether this frame asked for no next one.
    pub settled: bool,
    /// Each effect's time in the frame, by `EffectName`.
    pub effects: [f64; EFFECT_CNT],
    /// The requests' pump's time, and each part of the stage's, by
    /// `StagePartName`.
    pub pump_ms: f64,
    pub stage_parts: [f64; STAGE_PART_CNT],
    /// The invalidations for a frame since the frame before, by `InvalidationReason`
    /// (sys/sched.rs `take_invalidations`).
    pub invalidations: [u32; INVALIDATION_REASON_CNT],
}

/// Records the frame once its frame is drawn. What taking the record
/// costs counts as the profiler's time (`perf_ms`), not the frame task's.
pub(crate) fn perf_frame(t: CpuSample) {
    let at = now();
    // The peaks and counts are taken whether or not the profiler records, so
    // a recording that starts later counts from its own start.
    let heap = heap::stat();
    let (loads, refusals) = cache_cnts();
    let section = peaks(&heap, refusals);
    let renderer = core::mem::take(RENDERER.get(RendererFrame::default));
    if !perf_recording() {
        return;
    }
    if let Some(r) = renderer.gpu_read {
        perf_record_gpu(r);
    }
    if let Some(r) = renderer.ray_read {
        perf_record_rays(r);
    }
    perf_fold_recording(&section);
    let rec = FrameSample {
        // The time the stage ended, before the record was taken.
        t: at,
        refresh_ms: t.refresh_ms,
        paced_ms: t.paced_ms,
        settled: f64::from(u8::from(t.settled)),
        budget_ms: t.tick.budget_ms,
        overran: f64::from(u8::from(t.tick.overran)),
        tick_ms: t.tick.total_ms,
        phases: core::array::from_fn(|k| t.tick.phase_ms[k]),
        pump_ms: t.pump_ms,
        stage_parts: t.stage_parts,
        effects: t.effects,
        invalidations: t.invalidations.map(f64::from),
        kind: f64::from(renderer.kind as u8),
        gpu: renderer.gpu,
        loads,
        device: renderer.device,
        heap_used_bytes: heap.peak as f64,
        // The interval's figures come later: its frames' as each ends, and its
        // length when the next frame closes it.
        ..FrameSample::default()
    };
    let own_ms = now() - at;
    perf_record_frame(
        FrameSample {
            perf_ms: own_ms,
            ..rec
        },
        t.gap_ms,
    );
}
