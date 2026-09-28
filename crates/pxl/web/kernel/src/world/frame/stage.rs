// pxl-kernel/world/frame/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view
//! (view.rs) and the settings, and what the view's renderer is handed for the
//! frame (pxl-render's gpu/renderer.rs) — the draw list the scheduler left,
//! the environment, the models and skins as they stand — with what it hands
//! back: whether another turn is wanted, and what the frame cost, for the
//! profiler. The renderer reads nothing of the kernel on its own.

use pxl_render::frame::stage::{stage_lane_mut, AO_STRENGTH, EDGE_STRENGTH, EDGE_WIDTH, STAGE_AT};
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, FrameInput};
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::space::lod::split_distance;

use crate::layer::billboard::distance_fade::fade_range;
use pxl_spec::asset::ingested::Asset;
use pxl_spec::space::mat4::{mat4, mat4_mul};
use pxl_spec::space::pvf3::ivec;
use xpute_core::math::scalar::round;

use crate::host::handheld;
use crate::io::load::{load_image, Load};
use crate::kernel::kernel;
use crate::layer::billboard::sheet::billboard_sheet;
use crate::layer::character::companion::companion_skins;
use crate::layer::character::preview::preview_rows;
use crate::layer::decor::asset_store::decor_bundle_assets;
use crate::layer::environment::config::rim_cfg;
use crate::sched::profiler::record::{DeviceSample, DrawCensus, GpuSample, CENSUS_LEN};
use crate::shell::setting::store::{settings_store, AoMode};
use crate::text::text_raster;
use crate::world::change::request_world_tick;
use crate::world::chunk::stage::{set_host_frame_stat, HostFrameStat};
use crate::world::frame::resolution::resolution_observe;
use crate::world::frame::view::view;

const MB: f64 = 1_048_576.0;

/// Writes the frame's uniforms from the view and the settings as they stand,
/// the world drawn at `resolution` device pixels to a CSS pixel.
pub fn stage_write(resolution: f64) {
    let v = view();
    let settings = settings_store();
    let s = stage_lane_mut();
    let mut m = mat4();
    mat4_mul(&mut m, &v.view_matrix, &v.root);
    s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].copy_from_slice(&m);
    s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16].copy_from_slice(&v.proj);
    s[STAGE_AT::LOCAL_VIEW_PROJ..STAGE_AT::LOCAL_VIEW_PROJ + 16].copy_from_slice(&v.local_view_proj);
    s[STAGE_AT::EYE_LOCAL..STAGE_AT::EYE_LOCAL + 3].copy_from_slice(&v.eye_local);
    s[STAGE_AT::TRANSLATION..STAGE_AT::TRANSLATION + 3].copy_from_slice(&[v.tx, v.ty, v.tz]);
    s[STAGE_AT::ORIGIN] = ivec::to_real(ivec::X(v.origin));
    s[STAGE_AT::ORIGIN + 1] = ivec::to_real(ivec::Y(v.origin));
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = v.fov;
    s[STAGE_AT::NEAR] = v.near;
    s[STAGE_AT::FAR] = v.far;
    (s[STAGE_AT::FADE_START], s[STAGE_AT::FADE_END]) = fade_range();
    // The largest drawn cell per unit of distance: past a level's split
    // distance its cells are drawn averaged 2 × 2, so a cell there is twice
    // the level's over the distance, and half that just before the next
    // split. For whoever has to tolerate a drawn cell's height — the badge
    // occlusion test.
    s[STAGE_AT::LOD_CELL] = 2.0 / split_distance(0);
    s[STAGE_AT::EDGE_STRENGTH] = if settings.edge_shade { EDGE_STRENGTH } else { 0.0 };
    s[STAGE_AT::AO_STRENGTH] = if settings.ao == AoMode::corner { AO_STRENGTH } else { 0.0 };
    s[STAGE_AT::SCREEN_AO] = if settings.ao == AoMode::screen { 1.0 } else { 0.0 };
    s[STAGE_AT::LOW_SPEC] = if settings.textures { 0.0 } else { 1.0 };
    s[STAGE_AT::EDGE_WIDTH] = EDGE_WIDTH;
    s[STAGE_AT::RIM_INTENSITY] = if settings.rim_light { rim_cfg::INTENSITY } else { 0.0 };
    s[STAGE_AT::SHADOWS] = if settings.shadows { 1.0 } else { 0.0 };
    s[STAGE_AT::RESOLUTION] = resolution;
}

/// The turn's draw: the stage's frame handed to the renderer with the
/// scheduler's lists and what the frame's passes read, and what it reports
/// taken back.
pub fn stage_render() {
    let lists = kernel().sched.draw.take();
    let models = &kernel().sched.models;
    let mut decor = [(0u32, None); GROUPS];
    let variants = decor_bundle_assets()
        .iter()
        .flat_map(|entry| (0..entry.models.len()).map(move |v| (entry.models[v], entry.skins.get(v).copied().flatten())));
    for (slot, (model, image)) in decor.iter_mut().zip(variants) {
        *slot = (model.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle), image);
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
    set_host_frame_stat(HostFrameStat {
        task_ms: 0.0,
        gpu: GpuSample {
            ms: stat.gpu_ms,
            spans: stat.gpu_spans,
            draw_calls: stat.draw_calls,
            triangles: stat.triangles,
            census,
            level_tris: Default::default(),
            px: stat.px,
        },
        device: DeviceSample {
            buffers: stat.buffers,
            buffer_mb: round(stat.buffer_bytes / MB),
            arena_mb: stat.arena_mb,
            depth_mb: stat.depth_mb,
            shadow_mb: stat.shadow_mb,
            layers_mb: stat.layers_mb,
        },
    });
}
