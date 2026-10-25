// pxl-kernel/domain/scene/stage.rs

//! The kernel's side of a drawn frame: the stage written from the view and
//! the settings, and everything the renderer is handed, since it reads
//! nothing of the kernel on its own.

use crate::domain::scene::frame::Frame;
use crate::sys::sched::tick::TickContext;
use pxl_render::frame::stage::{stage_lane, STAGE_AT};
use pxl_render::gpu::atmosphere::EARTH_LAYERS;
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::gpu::renderer::{render_frame, Cast, EarthTiles, FrameInput, GroundSquares};
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::space::coord::v3;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;

use crate::profiler::frame::{DeviceSample, GpuSample};
use pxl_render::gpu::api::OBJECT_OPS;
use pxl_spec::asset::ingested::Asset;

use crate::domain::layer::earth::{earth_upload, earth_window, Earth, EARTH_FRAMES};
use crate::domain::query::decor::asset::decor_assets;
use crate::domain::query::model::draw_list;
use crate::domain::scene::character::companion::companion_skins;
use crate::domain::scene::character::preview::preview_rows;
use crate::domain::scene::environment::cloud::cloud_groups;
use crate::domain::scene::indicator::coverage_gizmo::coverage_gizmo_step;
use crate::domain::scene::indicator::frame_gizmo::frame_gizmo_step;
use crate::domain::scene::marker::sheet;
use crate::domain::scene::scene;
use crate::domain::view::demand::Observed as _;
use crate::domain::view::setting::store::{settings_store, AoMode};
use crate::profiler::report::set_host_frame;
use crate::sys::io::loader::load_image;
use crate::sys::io::text::text_raster;
use crate::sys::load::Load;
use crate::sys::mem::section;
use crate::sys::sched::edge::request_world_tick;

/// The device the last frame went to the screen on (`device_generation`).
static DRAWN: Global<Option<u32>> = Global::new();

/// Whether the screen holds a frame of the world: one drawn on the device
/// there is, as a device that went took what it drew.
pub fn screen_drawn() -> bool {
    *DRAWN.get(|| None) == Some(pxl_render::gpu::api::device_generation())
}

/// Blue Marble's July of 2004: one picture of the Earth at every hour and
/// season, as Google Earth and Cesium keep one mosaic; the earth layer's, under
/// its version.
const EARTH: Load = Load::Sphere;

use crate::domain::view::frame::pose::{sea_under_eye, view};

const _: () = assert!(EARTH_FRAMES == EARTH_LAYERS, "a GPU layer for each frame the earth layer holds");

pub fn stage_write() {
    let v = view();
    let settings = settings_store();
    let s = stage_lane();
    // The GPU gets the eye's frame, where the draws' points are.
    let (view_proj, camera) = v.eye_frame();
    s[STAGE_AT::MARKER_VIEW..STAGE_AT::MARKER_VIEW + 16].copy_from_slice(&camera);
    s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16].copy_from_slice(&v.proj);
    s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16].copy_from_slice(&view_proj);
    s[STAGE_AT::EYE..STAGE_AT::EYE + 3].copy_from_slice(&v.at().eye);
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = crate::domain::view::param::FOV;
    // The camera grid's cell angle: the footprint a drawn block covers per
    // unit of distance, square on.
    let tau = crate::domain::view::lens::tau();
    s[STAGE_AT::LOD_CELL] = crate::domain::view::lens::cell_angle(tau);
    s[STAGE_AT::EDGE_STRENGTH] = if settings.edge_shade { crate::domain::scene::param::EDGE_STRENGTH } else { 0.0 };
    s[STAGE_AT::SCREEN_AO] = if settings.ao == AoMode::screen { 1.0 } else { 0.0 };
    s[STAGE_AT::CORNER_AO] = if settings.ao == AoMode::corner { crate::domain::scene::param::AO_STRENGTH } else { 0.0 };
    s[STAGE_AT::LOW_SPEC] = if settings.textures { 0.0 } else { 1.0 };
    // In units, so neither the resolution nor a block's size moves it.
    s[STAGE_AT::EDGE_WIDTH] = crate::domain::scene::param::EDGE_WIDTH;
    s[STAGE_AT::RIM_INTENSITY] = if settings.rim_light { crate::domain::scene::param::RIM_INTENSITY } else { 0.0 };
    s[STAGE_AT::CLOUDS] = crate::domain::scene::environment::cloud::clouds_shown();
    // Out to where the grid's cells one zoom coarser than the ground's leaf begin.
    s[STAGE_AT::SHADOWS] = if settings.shadows { tau.split_distance(GROUND.cell() - 1) } else { 0.0 };
}

/// `pace` is the frame as the turn paced it, which the world's drawn
/// resolution is judged by (domain/view/frame/resolution.rs `load`).
pub fn stage_render(pace: crate::domain::view::frame::resolution::Pace) {
    // Nothing is drawn before the floor: its arrival asks for the next turn.
    if load_image(EARTH).is_none() {
        return;
    }
    let lists = draw_list().take();
    let models = crate::domain::query::model::models();
    let mut decor = [(0u32, None, [0.0f32; 3]); GROUPS];
    let variants = decor_assets().iter().flat_map(|entry| {
        (0..entry.models.len()).map(move |v| {
            let extent = entry.decor.variants.get(v).and_then(|a| a.model.ingested().extent).unwrap_or([0.0; 3]);
            (entry.models[v], entry.skin(v), extent.map(|e| e as f32))
        })
    });
    for (slot, (model, image, extent)) in decor.iter_mut().zip(variants) {
        *slot = (model.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle), image, extent);
    }
    for (group, mesh, image, extent) in cloud_groups() {
        decor[group] = (mesh, image, extent);
    }
    let none: (u32, &'static [u8]) = (0, &[]);
    let mut skins = [(0u32, none); 2];
    let mut skin_cnt = 0;
    for (slot, row) in skins.iter_mut().zip(companion_skins()) {
        *slot = row;
        skin_cnt += 1;
    }
    let mut cases = [(0u32, 0u32, none); SURFACES];
    let mut case_cnt = 0;
    for (slot, row) in cases.iter_mut().zip(preview_rows()) {
        *slot = row;
        case_cnt += 1;
    }
    let on = pxl_render::gpu::api::device_generation();
    let (window, layers) = earth_window(Earth::demand().floor(), on);
    let mut earth_upload = move |write: &mut dyn FnMut(u32, &[u8]) -> bool| earth_upload(write, on);
    let o = crate::domain::view::observation::observation();
    let eye = o.as_ref().and_then(crate::domain::scene::cast::eye);
    let mut terrain_upload = crate::domain::scene::cast::upload;
    let mut terrain_landed = crate::domain::layer::terrain::window::landed;
    let out = render_frame(FrameInput {
        lists,
        environment: &scene().environment.out,
        decor: &decor,
        skins: &skins[..skin_cnt],
        cases: &cases[..case_cnt],
        pack: || load_image(Load::Pack),
        moon: || load_image(Load::Asset(Asset::sky_moon_png)),
        earth: || load_image(EARTH),
        earth_tiles: EarthTiles {
            window,
            layers: &layers,
            upload: &mut earth_upload,
        },
        ground: GroundSquares {
            upload: &mut terrain_upload,
            landed: &mut terrain_landed,
            joined: crate::domain::layer::terrain::window::joined_lane(),
            pool: [crate::domain::scene::param::WAYS_POOL_FIRST_BYTES, crate::domain::layer::terrain::param::WAYS_POOL_BYTES].map(|b| b / (4 * pxl_spec_core::ways::tree::BLOCK_WORDS)),
            scratch: [crate::domain::scene::param::WAYS_SCRATCH_FIRST_BYTES, crate::domain::scene::param::WAYS_SCRATCH_BYTES]
                .map(|b| pxl_spec_core::ways::tree::scratch_blocks(b / (4 * pxl_spec_core::window::HANDED))),
            records: crate::domain::scene::param::WAYS_RECORDS_FIRST_BYTES,
        },
        cast: o.as_ref().and(eye).map(|eye| Cast {
            eye,
            window: crate::domain::layer::terrain::window::table(),
            shards: crate::domain::scene::cast::shards(),
            overwrites: crate::domain::scene::cast::overwrites(),
            decor: crate::domain::scene::decor::drawn::frame(),
        }),
        text: text_raster,
        sheet: sheet(),
        admit: crate::sys::io::host::admit_frame,
        // The draws are the profiler's, which records only while its setting is on.
        counted: settings_store().profiler,
    });
    if out.redraw {
        request_world_tick();
    }
    // An image whose texture the frame took needs its file no more.
    pxl_render::gpu::api::take_landed(crate::sys::io::loader::image_landed);
    // What the renderer holds, drawn this frame or not.
    let held = pxl_render::gpu::api::gpu().map_or([0; OBJECT_OPS], |g| *g.objects());
    let objects: [f64; OBJECT_OPS - 1] = core::array::from_fn(|k| f64::from(held[k + 1]));
    // The scale the world was drawn at, before a read can move the next one.
    let scale = crate::domain::view::frame::resolution::drawn_scale();
    let gpu = out
        .stat
        .as_ref()
        .filter(|stat| stat.gpu_read)
        .map(|stat| (pxl_render::gpu::renderer::pixels_ms(&stat.gpu_spans), stat.gpu_px));
    crate::domain::view::frame::resolution::load(pace, gpu);
    let Some(stat) = out.stat else {
        set_host_frame(out.kind, GpuSample { scale, ..GpuSample::default() }, DeviceSample { objects, ..DeviceSample::default() });
        return;
    };
    DRAWN.set(Some(pxl_render::gpu::api::device_generation()));
    set_host_frame(
        out.kind,
        GpuSample {
            read: f64::from(u8::from(stat.gpu_read)),
            spans: stat.gpu_spans,
            px: stat.gpu_px,
            scale,
            census: stat.census,
        },
        DeviceSample {
            blocks: stat.blocks,
            blocks_bytes: stat.block_bytes,
            depth_bytes: stat.depth_bytes,
            pack_bytes: stat.pack_bytes,
            objects,
        },
    );
}

struct Pass {
    draw: bool,
}

static PASS: Global<Pass> = Global::new();

fn pass() -> &'static mut Pass {
    PASS.get(|| Pass { draw: false })
}

pub fn take_draw() -> bool {
    core::mem::take(&mut pass().draw)
}

/// The scene's turn, `focus` and `heading` the camera's as the turn hands
/// them (pxl-kernel turn.rs `scene_tick`), for the world gizmo.
pub fn visible_step(ctx: &mut TickContext, focus: Option<pxl_spec::space::coord::q3::Q3>, heading: Option<f64>) {
    let c = pass();
    let v = view();

    let anchor = sea_under_eye();
    let e = v.at().eye;
    let frame = Frame {
        anchor,
        eye: e,
        frustum: v.frustum.about(anchor),
        camera: v3::sub(&e, &anchor),
        delta_s: ctx.delta,
    };

    // The world gizmo, laid before the frame's list begins, so its lines and the
    // origin they are drawn about are the same turn's: laid after, a new lane
    // went up under the last origin, and every rebuild flung the lines by how
    // far it moved.
    let show = settings_store().world_gizmo;
    frame_gizmo_step(show, focus, heading);
    coverage_gizmo_step(show);
    let draw = section::map().is_some_and(|_| {
        let (scene, list) = (scene(), draw_list());
        let models = crate::domain::query::model::models();
        let animating = scene.tick(list, models, &frame);
        scene.begin(list, models, frame.eye);
        crate::domain::scene::indicator::camera_gizmo::draw(list, settings_store().camera_gizmo);
        animating
    });
    {
        let edits = crate::domain::layer::object::preview::current();
        crate::domain::scene::decor::drawn::step(frame.eye, edits.hide.filter(|_| edits.hide_in.1), edits.scattered);
        crate::domain::scene::environment::cloud::step(draw_list(), &frame, &ctx.budget);
    }
    crate::domain::scene::marker::step(ctx, &frame);
    crate::domain::scene::marker::draw(draw_list(), frame.eye);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
}
