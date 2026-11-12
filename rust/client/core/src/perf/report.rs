// pxl-core/perf/report.rs

//! Builds a frame turn's record once its frame is drawn, so every figure in
//! it belongs to that one turn: what the turn measured of itself, what the
//! renderer made of its frame, and the terrain tiles it took in.

use crate::sys::sched::tick::TickReport;
use pxl_render::gpu::span_spec::FrameKind;
use pxl_render::renderer::GpuRead;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::mem::heap::HeapStat;

use crate::domain::layer::Base;
use crate::domain::scene::stage::FrameReport;
use crate::perf::frame::{DeviceSample, FrameSample, GpuSample, RaySample};
use crate::perf::record::{
    perf_fold_recording, perf_record_frame, perf_record_gpu, perf_record_rays, perf_recording,
};
use crate::perf::section::RecordingSection;
use crate::sys::boot;
use crate::sys::io::body;
use crate::turn;

/// What the renderer made of this turn's frame (domain/scene/stage.rs
/// `stage_render`), with the reads of earlier frames that landed this turn.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RendererFrame {
    kind: FrameKind,
    gpu: GpuSample,
    device: DeviceSample,
    gpu_read: Option<GpuRead>,
    ray_read: Option<RayRead>,
}

/// A frame's ray counts as they land, with the renderer's number for the
/// frame that counted them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayRead {
    pub frame: f64,
    pub sample: RaySample,
}

impl Default for RendererFrame {
    fn default() -> Self {
        RendererFrame {
            kind: FrameKind::none,
            gpu: GpuSample::default(),
            device: DeviceSample::default(),
            gpu_read: None,
            ray_read: None,
        }
    }
}

static RENDERER: Global<RendererFrame> = Global::new();

/// Takes the renderer's report for this turn's frame (domain/scene/stage.rs
/// `stage_render`) and the reads of earlier frames that landed this turn.
/// The figures are 0 when the renderer drew no frame.
pub fn set_renderer_frame(report: &FrameReport) {
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
    let [draws, triangles] = stat
        .draw_stat
        .iter()
        .fold([0.0; 2], |[d, t], c| [d + c[0], t + c[1]]);
    RENDERER.set(RendererFrame {
        kind,
        gpu: GpuSample {
            scale,
            draws,
            triangles,
            frame: stat.frame,
            light_changed: stat.light_changed,
            decor_rects: stat.decor_rects,
            ..GpuSample::default()
        },
        device: DeviceSample {
            uploads: stat.uploads,
            uploads_bytes: stat.upload_bytes,
            pack_bytes: stat.pack_bytes,
        },
        gpu_read: stat.gpu,
        ray_read: stat.ray_cnt.map(|r| RayRead {
            frame: r.frame,
            sample: RaySample::of(r.cnts),
        }),
    });
}

/// What this frame turn held, for the recording's extremes: the heap as its
/// allocator reports it, the body slots' peak since the frame before, and the terrain tiles refused a slot since then. Taking a peak or
/// a count starts it again from zero.
fn peaks(h: &HeapStat) -> RecordingSection {
    RecordingSection {
        frames: 1.0,
        heap_live_max_bytes: h.live as f64,
        heap_run_max_bytes: h.run as f64,
        heap_slot_max_bytes: h.slot as f64,
        heap_range_max_bytes: h.range as f64,
        heap_largest_free_min_bytes: h.largest_free as f64,
        heap_refusals: f64::from(h.refusals),
        body_slots_peak: f64::from(body::take_peak()),
        terrain_refusals: TerrainTile::tile_cache().take_refusals() as f64,
    }
}

/// A frame's own measures (turn.rs `world_frame`).
pub struct FrameTurn {
    pub tick: TickReport,
    /// The display's measured interval, and the gap since the last frame,
    /// which closes the interval before this one and which this frame was
    /// paced at.
    pub refresh_ms: f64,
    pub paced_ms: f64,
    /// Whether this frame asked for no next one.
    pub settled: bool,
    pub effects_ms: f64,
    pub stage_ms: f64,
}

/// Records the frame turn once its frame is drawn. What taking the record
/// costs counts as the profiler's time (`perf_ms`), not the frame task's.
pub fn perf_frame(t: FrameTurn) {
    let at = now();
    // The peaks and counts are taken whether or not the profiler records, so
    // a recording that starts later counts from its own start.
    let heap = boot::heap_stat();
    let section = peaks(&heap);
    let terrain_loads = f64::from(TerrainTile::tile_cache().take_loads());
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
        budget_ms: t.tick.budget_ms,
        effects_ms: t.effects_ms,
        tick_ms: t.tick.total_ms,
        // The core's phases fill the first of the slots xpute's tick has room
        // for.
        phases: core::array::from_fn(|k| t.tick.phase_ms[k]),
        stage_ms: t.stage_ms,
        effects: turn::effect_times(),
        kind: f64::from(renderer.kind as u8),
        gpu: renderer.gpu,
        terrain_loads,
        device: renderer.device,
        heap_used_bytes: heap.peak as f64,
        // The interval's figures come later: its turns' as each ends, and its
        // length when the next frame closes it.
        ..FrameSample::default()
    };
    let own_ms = now() - at;
    perf_record_frame(
        FrameSample {
            perf_ms: own_ms,
            ..rec
        },
        t.paced_ms,
        t.settled,
    );
}
