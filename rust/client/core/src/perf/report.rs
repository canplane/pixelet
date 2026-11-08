// pxl-core/perf/report.rs

//! A frame turn's record: what the turn measured of itself, what the renderer
//! made of its frame, and the terrain tiles it took in, taken once its frame
//! is drawn, so every figure in it is that one turn's.

use pxl_render::gpu::renderer::GpuRead;
use pxl_render::gpu::span_spec::FrameKind;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::mem::heap::HeapStat;
use xpute_core::sched::tick::TickReport;

use crate::domain::layer::Base;
use crate::domain::scene::stage::FrameReport;
use crate::door;
use crate::perf::frame::{DeviceSample, FrameSample, GpuSample, RaySample};
use crate::perf::record::{
    perf_fold_recording, perf_record_frame, perf_record_gpu, perf_record_rays, perf_recording,
};
use crate::perf::section::RecordingSection;
use crate::sys::boot;
use crate::sys::io::body;
use crate::turn;

/// What the renderer made of this turn's frame (domain/scene/stage.rs
/// `stage_render`), and the reads of earlier frames, landed now.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RendererFrame {
    kind: FrameKind,
    gpu: GpuSample,
    device: DeviceSample,
    gpu_read: Option<GpuRead>,
    rays: Option<RayRead>,
}

/// A frame's ray counts as they land: the renderer's number of the frame
/// that counted them, and the counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayRead {
    pub frame: f64,
    pub rays: RaySample,
}

impl Default for RendererFrame {
    fn default() -> Self {
        RendererFrame {
            kind: FrameKind::none,
            gpu: GpuSample::default(),
            device: DeviceSample::default(),
            gpu_read: None,
            rays: None,
        }
    }
}

static RENDERER: Global<RendererFrame> = Global::new();

/// The renderer's answer for this turn's frame (domain/scene/stage.rs
/// `stage_render`), its figures zero where it drew none, and the reads of
/// earlier frames, landed now.
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
        .draws
        .iter()
        .fold([0.0; 2], |[d, t], c| [d + c[0], t + c[1]]);
    RENDERER.set(RendererFrame {
        kind,
        gpu: GpuSample {
            scale,
            draws,
            triangles,
            frame: stat.frame,
            light_moved: stat.light_moved,
            decor_boxes: stat.decor_boxes,
            ..GpuSample::default()
        },
        device: DeviceSample {
            uploads: stat.uploads,
            uploads_bytes: stat.upload_bytes,
            pack_bytes: stat.pack_bytes,
        },
        gpu_read: stat.gpu,
        rays: stat.rays.map(|r| RayRead {
            frame: r.frame,
            rays: RaySample::of(r.counts),
        }),
    });
}

/// What this frame turn held, folded into the recording's extremes: the heap as
/// its allocator states it, the rings' and body slots' peaks since the frame
/// before, which taking them starts again, and the terrain tiles refused a
/// slot since then.
fn held_now(h: &HeapStat) -> RecordingSection {
    let rings = door::take_stat();
    RecordingSection {
        frames: 1.0,
        heap_live_max_bytes: h.live as f64,
        heap_run_max_bytes: h.run as f64,
        heap_slot_max_bytes: h.slot as f64,
        heap_range_max_bytes: h.range as f64,
        heap_largest_free_min_bytes: h.largest_free as f64,
        heap_refusals: f64::from(h.refusals),
        ring_cq_peak: f64::from(rings.cq_peak),
        ring_sq_peak: f64::from(rings.sq_peak),
        ring_left_peak: f64::from(rings.left_peak),
        body_slots_peak: f64::from(body::take_peak()),
        cap_terrain_refused: TerrainTile::tile_cache().take_refusals() as f64,
    }
}

/// A frame turn's own measures (turn.rs `world_frame`).
pub struct FrameTurn {
    pub tick: TickReport,
    /// The runtime's gap that ended at this frame, which closes the interval
    /// before it, and whether the runtime settled it, as it says with this frame.
    pub interval_ms: f64,
    /// The display's refresh and the interval the runtime paced this frame at
    /// (turn.rs `TurnGrant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub settled: bool,
    pub effects_ms: f64,
    pub stage_ms: f64,
    /// The frame task's whole time, which the turn's end takes out of its tasks'.
    pub task_ms: f64,
}

/// The frame turn's record, once its frame is drawn: what taking it cost
/// is the profiler's (`perf_ms`), out of the frame task's time.
pub fn perf_frame(t: FrameTurn) {
    let at = now();
    // Taken whether or not recording, so a recording starts from now.
    let heap = boot::heap_stat();
    let held = held_now(&heap);
    let terrain_landed = f64::from(TerrainTile::tile_cache().take_loads());
    let renderer = core::mem::take(RENDERER.get(RendererFrame::default));
    if !perf_recording() {
        return;
    }
    if let Some(r) = renderer.gpu_read {
        perf_record_gpu(r);
    }
    if let Some(r) = renderer.rays {
        perf_record_rays(r);
    }
    perf_fold_recording(&held);
    let record = FrameSample {
        // As the stage ended, before the record is taken.
        t: at,
        refresh_ms: t.refresh_ms,
        paced_ms: t.paced_ms,
        budget_ms: t.tick.budget_ms,
        effects_ms: t.effects_ms,
        tick_ms: t.tick.total_ms,
        // The core's phases, the first of what xpute's tick leaves room for.
        phases: core::array::from_fn(|k| t.tick.phase_ms[k]),
        stage_ms: t.stage_ms,
        effects: turn::effect_times(),
        kind: f64::from(renderer.kind as u8),
        gpu: renderer.gpu,
        terrain_landed,
        device: renderer.device,
        heap_used_bytes: heap.peak as f64,
        // The interval's: its turns' as they end, and its length as the next frame closes it.
        ..FrameSample::default()
    };
    let own_ms = now() - at;
    perf_record_frame(
        FrameSample {
            perf_ms: own_ms,
            ..record
        },
        t.interval_ms,
        t.settled,
        t.task_ms + own_ms,
    );
}
