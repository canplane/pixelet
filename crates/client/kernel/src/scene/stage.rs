// pxl-kernel/scene/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view and
//! the settings, and everything the renderer is handed, since it reads
//! nothing of the kernel on its own.

use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::with_world;
use crate::world::chunk::decor::build::DecorHide;
use crate::world::chunk::table::Frame;
use crate::world::view::frame::pose::{globe, view_resize};
use pxl_render::frame::stage::{stage_lane_mut, STAGE_AT};
use pxl_render::gpu::atmosphere::EARTH_LAYERS;
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, EarthTiles, FrameInput};
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::space::coord::v3::sub3;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::mem::MB;
use crate::sched::profiler::frame::{DeviceSample, DrawCensus, GpuSample, CENSUS_LEN};
use pxl_spec::asset::ingested::Asset;
use xpute_core::math::scalar::round;

use crate::door::host::handheld;
use crate::io::load::{load_image, Load};
use crate::io::text::text_raster;
use crate::kernel;
use crate::layer::earth::{earth_upload, earth_window, Earth, EARTH_FRAMES};
use crate::scene::billboard::sheet::billboard_sheet;
use crate::scene::character::companion::companion_skins;
use crate::scene::character::preview::preview_rows;
use crate::sched::edge::request_world_tick;
use crate::sched::profiler::report::set_host_frame_stat;
use crate::shell::setting::store::{settings_store, AoMode};
use crate::world::chunk::decor::asset_store::kind_assets;
use crate::world::pyramid::Observed as _;
use crate::world::view::frame::resolution::resolution_observe;

/// Blue Marble's July of 2004: one picture of the Earth at every hour and
/// season, as Google Earth and Cesium keep one mosaic.
const EARTH: Asset = Asset::sky_earth_07_webp;

use crate::world::view::frame::pose::{sea_under_eye, view};

const _: () = assert!(EARTH_FRAMES == EARTH_LAYERS, "a GPU layer for each frame the Earth layer holds");

pub fn stage_write() {
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
    s[STAGE_AT::EDGE_STRENGTH] = if settings.edge_shade { crate::param::EDGE_STRENGTH } else { 0.0 };
    s[STAGE_AT::AO_STRENGTH] = if settings.ao == AoMode::corner { crate::param::AO_STRENGTH } else { 0.0 };
    s[STAGE_AT::SCREEN_AO] = if settings.ao == AoMode::screen { 1.0 } else { 0.0 };
    s[STAGE_AT::LOW_SPEC] = if settings.textures { 0.0 } else { 1.0 };
    s[STAGE_AT::EDGE_WIDTH] = crate::param::EDGE_WIDTH;
    s[STAGE_AT::RIM_INTENSITY] = if settings.rim_light { crate::param::RIM_INTENSITY } else { 0.0 };
    s[STAGE_AT::SHADOWS] = if settings.shadows { 1.0 } else { 0.0 };
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
    let (window, layers) = earth_window(Earth::pyramid().floor());
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

struct Pass {
    visible_pass: Option<f64>,
    draw: bool,
}

static PASS: Global<Pass> = Global::new();

fn pass() -> &'static mut Pass {
    PASS.get(|| Pass { visible_pass: None, draw: false })
}

pub fn take_draw() -> bool {
    core::mem::take(&mut pass().draw)
}

pub fn visible_step(ctx: &mut TickContext) {
    let c = pass();
    c.visible_pass = None;
    view_resize(ctx.state.width, ctx.state.height);
    let v = view();

    let anchor = sea_under_eye();
    let e = v.pose.eye;
    let frame = Frame {
        anchor,
        eye: e,
        frustum: v.frustum.about(anchor),
        camera: sub3(&e, &anchor),
        delta_s: ctx.delta,
        remaining_ms: ctx.budget.remaining(),
        hard_slack_ms: HARD_SLACK_MS,
        globe: Some(globe()),
    };

    let t0 = now();
    // The chunks plan first; what animates and every layer but the chunks list
    // before the chunks draw.
    let draw = with_world(|_, sched, scene| {
        let moving = sched.tick(frame, &now);
        if let Some(d) = crate::world::chunk::domain::current() {
            scene.coverage.step(&mut scene.draw, settings_store().gizmos, d.pieces(), sched.drawn());
        }
        let animating = scene.tick(&frame);
        scene.begin(frame.eye);
        sched.draw(&frame, &mut scene.draw);
        moving || animating
    })
    .unwrap_or(false);
    // Decor lists after the chunks have listed theirs.
    {
        let k = kernel();
        let edits = crate::control::interaction::edit::render_edits();
        let hide = edits.hide.filter(|_| edits.hide_in.1).map(DecorHide::Placed).or(edits.scattered.map(DecorHide::Scattered));
        crate::world::chunk::decor::cell::step(&crate::world::chunk::decor::kind::KINDS, &k.objects, &k.sched, &mut k.scene.draw, hide, frame.eye, &ctx.budget);
    }
    crate::world::chunk::place::step(ctx);
    let scene = &mut kernel().scene;
    scene.billboards.draw(&mut scene.draw, &scene.models, frame.eye);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
    c.visible_pass = Some(now() - t0);
}

/// How long this turn's chunks took; none where the stage was empty.
pub fn visible_pass() -> Option<f64> {
    pass().visible_pass
}
