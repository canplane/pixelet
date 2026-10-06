// pxl-kernel/scene/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view and
//! the settings, and everything the renderer is handed, since it reads
//! nothing of the kernel on its own.

use pxl_render::frame::stage::{stage_lane_mut, STAGE_AT};
use pxl_render::gpu::atmosphere::EARTH_LAYERS;
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, EarthTiles, FrameInput};
use pxl_render::gpu::surface::SURFACES;

use crate::mem::MB;
use crate::sched::profiler::frame::{DeviceSample, DrawCensus, GpuSample, CENSUS_LEN};
use pxl_spec::asset::ingested::Asset;
use xpute_core::math::scalar::round;

use crate::host::handheld;
use crate::io::load::{load_image, Load};
use crate::kernel::kernel;
use crate::layer::earth::{earth_upload, earth_window, EARTH_FRAMES};
use crate::scene::billboard::sheet::billboard_sheet;
use crate::scene::character::companion::companion_skins;
use crate::scene::character::preview::preview_rows;
use crate::scene::environment::config::{ao_cfg, edge_cfg, rim_cfg};
use crate::shell::setting::store::{settings_store, AoMode};
use crate::text::text_raster;
use crate::world::change::request_world_tick;
use crate::world::chunk::decor::asset_store::kind_assets;
use crate::world::chunk::stage::set_host_frame_stat;
use crate::world::view::frame::resolution::resolution_observe;

/// Blue Marble's July of 2004: one picture of the Earth at every hour and
/// season, as Google Earth and Cesium keep one mosaic.
const EARTH: Asset = Asset::sky_earth_07_webp;

use crate::world::view::frame::pose::{sea_under_eye, view};

const _: () = assert!(EARTH_FRAMES == EARTH_LAYERS, "a GPU layer for each frame the Earth layer holds");

pub fn stage_write(resolution: f64) {
    let v = view();
    let settings = settings_store();
    let s = stage_lane_mut();
    // The GPU gets the eye's frame, where the draws' points are.
    let (view_proj, camera) = v.eye_frame();
    s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].copy_from_slice(&camera);
    s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16].copy_from_slice(&v.proj);
    s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16].copy_from_slice(&view_proj);
    s[STAGE_AT::EYE..STAGE_AT::EYE + 3].copy_from_slice(&v.pose.eye);
    s[STAGE_AT::GLOBE_ANCHOR..STAGE_AT::GLOBE_ANCHOR + 3].copy_from_slice(&sea_under_eye());
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = v.fov;
    s[STAGE_AT::NEAR] = v.near;
    s[STAGE_AT::FAR] = v.far;
    // Largest drawn cell per unit of distance, for the badge occlusion test:
    // a cell is drawn from its split distance on.
    s[STAGE_AT::LOD_CELL] = crate::world::view::observer::cell_angle(crate::world::chunk::quality::effective().tau());
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
    // Nothing is drawn before the floor: its arrival asks for the next turn.
    if load_image(Load::Asset(EARTH)).is_none() {
        return;
    }
    let lists = kernel().scene.draw.take();
    let models = &kernel().scene.models;
    let mut decor = [(0u32, None, [0.0f32; 3]); GROUPS];
    let variants = kind_assets().iter().flat_map(|entry| {
        (0..entry.models.len()).map(move |v| {
            let extent = entry.kind.variants.get(v).and_then(|a| a.model.ingested().extent).unwrap_or([0.0; 3]);
            (entry.models[v], entry.skin(v), extent.map(|e| e as f32))
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
    let (window, layers) = earth_window();
    let out = render_frame(FrameInput {
        lists,
        environment: &kernel().scene.environment.out,
        decor: &decor,
        skins: &skins[..skin_cnt],
        cases: &cases[..case_cnt],
        sheet: || load_image(Load::Sheet),
        moon: || load_image(Load::Asset(Asset::sky_moon_png)),
        earth: || load_image(Load::Asset(EARTH)),
        earth_tiles: EarthTiles {
            window,
            layers: &layers,
            upload: &mut earth_upload,
        },
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
            zoom_tris: Default::default(),
            px: stat.px,
            skipped: stat.skipped,
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
