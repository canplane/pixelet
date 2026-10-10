// pxl-kernel/scene/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view and
//! the settings, and everything the renderer is handed, since it reads
//! nothing of the kernel on its own.

use crate::chunk::decor::build::DecorHide;
use crate::chunk::table::Frame;
use crate::laid;
use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::view::frame::pose::{globe, view_resize};
use pxl_render::frame::stage::{stage_lane_mut, STAGE_AT};
use pxl_render::gpu::atmosphere::EARTH_LAYERS;
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, EarthTiles, FrameInput};
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::space::coord::v3::sub3;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::mem::MB;
use crate::profiler::frame::{DeviceSample, DrawCensus, GpuSample, CENSUS_LEN};
use pxl_spec::asset::ingested::Asset;
use xpute_core::math::scalar::round;

use crate::billboard::sheet::billboard_sheet;
use crate::decor::assets::kind_assets;
use crate::io::host::handheld;
use crate::io::load::{load_image, Load};
use crate::io::text::text_raster;
use crate::kernel;
use crate::layer::earth::{earth_upload, earth_window, Earth, EARTH_FRAMES};
use crate::profiler::report::set_host_frame_stat;
use crate::scene::character::companion::companion_skins;
use crate::scene::character::preview::preview_rows;
use crate::sched::edge::request_world_tick;
use crate::setting::store::{settings_store, AoMode};
use crate::view::demand::Observed as _;
use crate::view::frame::resolution::resolution_observe;

/// Blue Marble's July of 2004: one picture of the Earth at every hour and
/// season, as Google Earth and Cesium keep one mosaic.
const EARTH: Asset = Asset::sky_earth_07_webp;

use crate::view::frame::pose::{sea_under_eye, view};

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
    s[STAGE_AT::EYE..STAGE_AT::EYE + 3].copy_from_slice(&v.at().eye);
    s[STAGE_AT::GLOBE_ANCHOR..STAGE_AT::GLOBE_ANCHOR + 3].copy_from_slice(&sea_under_eye());
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = v.fov;
    s[STAGE_AT::NEAR] = v.near;
    s[STAGE_AT::FAR] = v.far;
    // The camera grid's cell angle: the largest drawn cell per unit of
    // distance, since a cell is drawn from its split distance on.
    s[STAGE_AT::LOD_CELL] = crate::view::sight::cell_angle(crate::chunk::quality::effective().tau());
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
    let lists = kernel().draw.take();
    let models = &kernel().models;
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
    let (window, layers) = earth_window(Earth::demand().floor());
    let out = render_frame(FrameInput {
        lists,
        environment: &kernel().scene.environment.out,
        decor: &decor,
        skins: &skins[..skin_cnt],
        cases: &cases[..case_cnt],
        pack: || load_image(Load::Pack),
        moon: || load_image(Load::Asset(Asset::sky_moon_png)),
        earth: || load_image(Load::Asset(EARTH)),
        earth_tiles: EarthTiles {
            window,
            layers: &layers,
            upload: &mut earth_upload,
        },
        text: text_raster,
        billboards: billboard_sheet(),
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
    let e = v.at().eye;
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
    crate::chunk::source::chunk_sources_step();
    let draw = laid().is_some_and(|k| {
        let moving = k.sched.tick(frame, &now);
        if let Some(d) = crate::chunk::domain::current() {
            k.scene.coverage.step(&mut k.draw, settings_store().world_gizmo, d.pieces(), k.sched.drawn());
        }
        let animating = k.scene.tick(&mut k.draw, &k.models, &frame);
        k.scene.begin(&mut k.draw, &k.models, frame.eye);
        crate::scene::marker::camera_gizmo::draw(&mut k.draw, settings_store().camera_gizmo);
        k.sched.draw(&frame, &mut k.draw);
        moving || animating
    });
    // Decor lists after the chunks have listed theirs.
    {
        let k = kernel();
        let edits = crate::chunk::source::previewed();
        let hide = edits.hide.filter(|_| edits.hide_in.1).map(DecorHide::Placed).or(edits.scattered.map(DecorHide::Scattered));
        crate::chunk::decor::cell::step(&pxl_spec::cell::decor::KINDS, &k.objects, &k.sched, &mut k.draw, hide, frame.eye, &ctx.budget);
    }
    crate::chunk::place::step(ctx);
    let k = kernel();
    let actors = &k.scene.actors;
    k.billboards.draw(&mut k.draw, &|id| actors.actor_ref(id).filter(|a| a.visible).map(|a| a.at), frame.eye);
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
