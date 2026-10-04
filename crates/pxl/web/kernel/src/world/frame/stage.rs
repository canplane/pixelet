// pxl-kernel/world/frame/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view and
//! the settings, and everything the renderer is handed, since it reads
//! nothing of the kernel on its own.

use pxl_render::frame::stage::{stage_lane_mut, STAGE_AT};
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, FrameInput};
use pxl_render::gpu::surface::SURFACES;

use crate::layer::billboard::distance_fade::fade_range;
use crate::sched::profiler::record::MB;
use pxl_spec::asset::ingested::Asset;
use pxl_spec::space::pvf3::ivec;
use xpute_core::math::scalar::round;

use crate::host::handheld;
use crate::io::load::{load_image, Load};
use crate::kernel::kernel;
use crate::layer::billboard::sheet::billboard_sheet;
use crate::layer::character::companion::companion_skins;
use crate::layer::character::preview::preview_rows;
use crate::layer::decor::asset_store::kind_assets;
use crate::layer::environment::config::rim_cfg;
use crate::sched::profiler::record::{DeviceSample, DrawCensus, GpuSample, CENSUS_LEN};
use crate::shell::setting::store::{settings_store, AoMode};
use crate::text::text_raster;
use crate::world::change::request_world_tick;
use crate::world::chunk::stage::set_host_frame_stat;
use crate::world::frame::resolution::resolution_observe;
use crate::world::frame::view::{globe_anchor_local, view};

/// Writes the frame's uniforms from the view and the settings as they stand,
/// the world drawn at `resolution` device pixels to a CSS pixel.
pub fn stage_write(resolution: f64) {
    let v = view();
    let settings = settings_store();
    let s = stage_lane_mut();
    // The GPU gets the eye's frame, where the draws' points are.
    let (view_proj, camera) = v.eye_frame();
    s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].copy_from_slice(&camera);
    s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16].copy_from_slice(&v.proj);
    s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16].copy_from_slice(&view_proj);
    s[STAGE_AT::EYE_LOCAL..STAGE_AT::EYE_LOCAL + 3].copy_from_slice(&v.pose.eye);
    s[STAGE_AT::GLOBE_ANCHOR..STAGE_AT::GLOBE_ANCHOR + 3].copy_from_slice(&globe_anchor_local());
    s[STAGE_AT::ORIGIN] = ivec::to_real(ivec::X(v.origin));
    s[STAGE_AT::ORIGIN + 1] = ivec::to_real(ivec::Y(v.origin));
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = v.fov;
    s[STAGE_AT::NEAR] = v.near;
    s[STAGE_AT::FAR] = v.far;
    // Largest drawn cell per unit of distance, for the badge occlusion test:
    // past a split distance cells are drawn 2 × 2, so twice the level's cell
    // over the distance.
    s[STAGE_AT::LOD_CELL] = 2.0 / crate::world::quality::effective().tau().split_distance(0);
    s[STAGE_AT::EDGE_STRENGTH] = if settings.edge_shade { edge_cfg::STRENGTH } else { 0.0 };
    s[STAGE_AT::AO_STRENGTH] = if settings.ao == AoMode::corner { ao_cfg::STRENGTH } else { 0.0 };
    s[STAGE_AT::SCREEN_AO] = if settings.ao == AoMode::screen { 1.0 } else { 0.0 };
    s[STAGE_AT::LOW_SPEC] = if settings.textures { 0.0 } else { 1.0 };
    s[STAGE_AT::EDGE_WIDTH] = edge_cfg::WIDTH;
    s[STAGE_AT::RIM_INTENSITY] = if settings.rim_light { rim_cfg::INTENSITY } else { 0.0 };
    s[STAGE_AT::SHADOWS] = if settings.shadows { 1.0 } else { 0.0 };
    s[STAGE_AT::RESOLUTION] = resolution;
}

pub fn stage_render() {
    let lists = kernel().sched.draw.take();
    let models = &kernel().sched.models;
    let mut decor = [(0u32, None, [0.0f32; 3]); GROUPS];
    let variants = kind_assets().iter().flat_map(|entry| {
        (0..entry.models.len()).map(move |v| {
            let extent = entry.kind.variants.get(v).and_then(|a| a.model.ingested().extent).unwrap_or([0.0; 3]);
            (entry.models[v], entry.skins.get(v).copied().flatten(), extent.map(|e| e as f32))
        })
    });
    for (slot, (model, image, extent)) in decor.iter_mut().zip(variants) {
        *slot = (model.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle), image, extent);
    }
    let mut skins = [(0u32, 0u32); 2];
    let mut skin_cnt = 0;
    for (slot, row) in skins.iter_mut().zip(companion_skins()) {
        *slot = row;
        skin_cnt += 1;
    }
    let mut cases = [(0u32, 0u32, 0u32); SURFACES];
    let mut case_cnt = 0;
    for (slot, row) in cases.iter_mut().zip(preview_rows()) {
        *slot = row;
        case_cnt += 1;
    }
    let out = render_frame(FrameInput {
        lists,
        environment: &kernel().sched.environment.out,
        decor: &decor,
        skins: &skins[..skin_cnt],
        cases: &cases[..case_cnt],
        sheet: || load_image(Load::Sheet),
        moon: || load_image(Load::Asset(Asset::sky_moon_png)),
        text: text_raster,
        atlas: billboard_sheet(),
        handheld: handheld(),
    });
    if out.redraw {
        request_world_tick();
    }
    let Some(stat) = out.stat else { return };
    resolution_observe(stat.gpu_ms);
    let mut census: DrawCensus = Default::default();
    census.copy_from_slice(&stat.census[..CENSUS_LEN]);
    set_host_frame_stat(
        GpuSample {
            ms: stat.gpu_ms,
            spans: stat.gpu_spans,
            draw_calls: stat.draw_calls,
            triangles: stat.triangles,
            census,
            level_tris: Default::default(),
            px: stat.px,
        },
        DeviceSample {
            buffers: stat.buffers,
            buffer_mb: round(stat.buffer_bytes / MB),
            arena_mb: stat.arena_mb,
            depth_mb: stat.depth_mb,
            shadow_mb: stat.shadow_mb,
            layers_mb: stat.layers_mb,
        },
    );
}
