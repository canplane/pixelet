// pxl-kernel/domain/scene/stage.rs

//! The kernel's side of a drawn frame: everything the renderer is handed, the
//! view and the settings among it, since it reads nothing of the kernel on
//! its own.

use crate::domain::scene::frame::Frame;
use crate::sys::sched::tick::TickContext;
use pxl_render::frame::draw::DRAW_WORDS;
use pxl_render::frame::shader::word::{DECOR_GROUPS, DRAW_FLOATS};
use pxl_render::gpu::atmosphere::EARTH_LAYERS;
use pxl_render::gpu::ray::{Cast, GroundBricks};
use pxl_render::gpu::renderer::{render_frame, EarthTiles, FrameInput, FrameStat};
use pxl_render::gpu::span_spec::FrameKind;
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::space::coord::v3;
use xpute_core::global::Global;

use crate::domain::layer::earth::{earth_upload, earth_window, Earth, EARTH_SLOTS};
use crate::domain::scene::character::companion::companion_skins;
use crate::domain::scene::character::surface::character_surface_rows;
use crate::domain::scene::decor::asset::{decor_model, decor_skin};
use crate::domain::scene::environment::cloud::cloud_groups;
use crate::domain::scene::environment::{environment_drift, environment_words};
use crate::domain::scene::gizmo::coverage_gizmo::coverage_gizmo_step;
use crate::domain::scene::gizmo::frame_gizmo::frame_gizmo_step;
use crate::domain::scene::marker::atlas;
use crate::domain::scene::model::draw_list;
use crate::domain::view::demand::Observed as _;
use crate::domain::view::setting::store::{settings_store, AoMode};
use crate::sys::io::loader::{catalog, load_image, load_of};
use crate::sys::load::Load;
use crate::sys::sched::edge::request_world_tick;

/// The device the last frame went to the screen on (`device_generation`).
static SCREEN_DEVICE: Global<Option<u32>> = Global::new();

/// Whether the screen holds a frame of the world: one drawn on the device
/// there is, as a device that went took what it drew.
pub fn is_screen_drawn() -> bool {
    *SCREEN_DEVICE.get(|| None) == Some(api::device_generation())
}

/// Blue Marble's July of 2004: one picture of the Earth at every hour and
/// season; the earth layer's, under its version.
const EARTH: Load = Load::Sphere;

use crate::domain::layer::terrain::clipmap;
use crate::domain::preview::{Draft, RenderEdits};
use crate::domain::scene;
use crate::domain::scene::animation;
use crate::domain::scene::cast;
use crate::domain::scene::decor::gpu;
use crate::domain::scene::environment::cloud;
use crate::domain::scene::marker;
use crate::domain::scene::model;
use crate::domain::scene::param;
use crate::domain::scene::resolution;
use crate::domain::scene::resolution::Pace;
use crate::domain::view::frame::pose::{sea_under_eye, view};
use crate::domain::view::frame::screen;
use crate::domain::view::observation;
use crate::sys::io::loader;
use pxl_engine::way::tree;
use pxl_render::gpu::api;
use pxl_spec::asset::catalog::pack;
use pxl_spec::asset::decor;
use pxl_spec::asset::sky::moon;
use pxl_spec::space::coord::q3;

const _: () = assert!(
    EARTH_SLOTS == EARTH_LAYERS,
    "a GPU layer for each frame the earth layer holds"
);

/// What the renderer made of the turn's frame, and the scale the world was
/// drawn at, read before a read can move the next: the top's to record (perf/report.rs `set_renderer_frame`).
pub struct Drawn {
    pub kind: FrameKind,
    pub stat: Option<FrameStat>,
    pub scale: f64,
    /// The interval the drawn resolution's judge asks the turns paced at
    /// (`resolution::load`), none where it asks no change.
    pub interval_ms: Option<f64>,
}

/// `pace` is the frame as the turn paced it, which the world's drawn
/// resolution is judged by (domain/scene/resolution.rs `load`), and
/// `edits` the pointer's preview as the turn hands it, which the rays cut by.
pub fn stage_render(pace: Pace, edits: &RenderEdits) -> Drawn {
    let v = view();
    let settings = settings_store();
    // The GPU gets the eye's frame, where the draws' points are.
    let (view_proj, rot) = v.eye_frame();
    let lists = draw_list().take();
    let stands = lists.stands;
    let models = model::models();
    // Each decor variant at its draw group (pxl-content scatter.rs `first_group`).
    let mut decor = [(0u32, None, [0.0f32; 3]); DECOR_GROUPS];
    for d in &decor::DECORS {
        for (v, variant) in d.variants.iter().enumerate() {
            let extent = catalog()
                .map_or([0.0; 3], |c| c.extent(variant.model))
                .map(|e| e as f32);
            let mesh = decor_model(d, v)
                .and_then(|m| models.mesh(m))
                .map_or(0, |m| m.handle);
            decor[d.first_group() as usize + v] = (mesh, decor_skin(d, v), extent);
        }
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
    let mut character_surfaces =
        [(0u32, ([0; DRAW_WORDS], [0.0; DRAW_FLOATS]), none, 0u32); SURFACES];
    let mut surface_cnt = 0;
    for (slot, row) in character_surfaces.iter_mut().zip(character_surface_rows()) {
        *slot = row;
        surface_cnt += 1;
    }
    let on = api::device_generation();
    let (window, layers) = earth_window(Earth::demand().floor(), on);
    let mut earth_upload = move |write: &mut dyn FnMut(u32, &[u8]) -> bool| earth_upload(write, on);
    let o = observation::observation();
    let eye = o.as_ref().and_then(|o| cast::eye(o, edits));
    let mut terrain_upload = clipmap::upload;
    // Whether what the scene animates stepped.
    let animated = animation::take();
    let out = render_frame(FrameInput {
        view: rot,
        proj: v.proj,
        view_proj,
        eye: v.at().eye,
        // The markers are drawn at the size the kernel lays them out at.
        px_per_unit: screen::px_per_unit(),
        edge_strength: if settings.edge_shade {
            param::EDGE_STRENGTH
        } else {
            0.0
        },
        rim_intensity: if settings.rim_light {
            param::RIM_INTENSITY
        } else {
            0.0
        },
        // The screen's occlusion takes the corners' too, which it reads softer.
        corner_ao: if matches!(settings.ao, AoMode::corner | AoMode::screen) {
            param::AO_STRENGTH
        } else {
            0.0
        },
        screen_ao: settings.ao == AoMode::screen,
        low_spec: !settings.textures,
        shadows: settings.shadows,
        clouds: cloud::clouds_shown(),
        drift: environment_drift(),
        lists,
        environment: environment_words(),
        decor: &decor,
        skins: &skins[..skin_cnt],
        character_surfaces: &character_surfaces[..surface_cnt],
        pack: load_of(pack).and_then(load_image),
        moon: load_of(moon::picture).and_then(load_image),
        earth: load_image(EARTH),
        earth_tiles: EarthTiles {
            window,
            layers: &layers,
            upload: &mut earth_upload,
        },
        ground: GroundBricks {
            upload: &mut terrain_upload,
            pool: clipmap::pool_room()
                .max(param::WAYS_POOL_FIRST_BYTES / (4 * tree::CHILD_GROUP_WORDS)),
        },
        cast: o.as_ref().and(eye).map(|eye| Cast {
            eye,
            clipmap: clipmap::table(),
            shards: cast::shards(),
            overwrites: cast::overwrites(),
            decor: gpu::frame(),
            stands,
            ray_counts: settings.ray_counts,
        }),
        text: marker::text_raster,
        atlas: atlas(),
        // The draws are the profiler's, which records only while its setting is on.
        counted: settings.perf,
        animated,
    });
    if out.redraw {
        request_world_tick();
    }
    // An image whose texture the frame took needs its file no more.
    api::take_landed(loader::on_image_landed);
    // The scale the world was drawn at, before a read can move the next.
    let scale = resolution::drawn_scale();
    let interval_ms = resolution::load(pace, out.stat.and_then(|stat| stat.gpu));
    if out.stat.is_some() {
        SCREEN_DEVICE.set(Some(api::device_generation()));
    }
    Drawn {
        kind: out.kind,
        stat: out.stat,
        scale,
        interval_ms,
    }
}

/// The scene's turn, as the turn hands it what control decided (pxl-kernel
/// turn.rs `scene_tick`): `pivot` and `heading` the camera's, for the world
/// gizmo, `edits` the pointer's preview, for decor, and `draft` the marker
/// tool's, for the markers.
pub fn visible_step(
    ctx: &mut TickContext,
    pivot: Option<q3::Q3>,
    heading: Option<f64>,
    edits: &RenderEdits,
    draft: Option<&Draft>,
) {
    let v = view();

    let anchor = sea_under_eye();
    let e = v.at().eye;
    let frame = Frame {
        anchor,
        eye: e,
        frustum: v.frustum.about(anchor),
        camera: v3::sub(&e, &anchor),
        px_per_unit: screen::px_per_unit() * resolution::dpr(),
        delta_s: ctx.delta,
    };

    // The world gizmo, laid before the frame's list begins, so its lines and the
    // origin they are drawn about are the same turn's.
    let show = settings_store().world_gizmo;
    frame_gizmo_step(show, pivot, heading);
    coverage_gizmo_step(show);
    let (list, models) = (draw_list(), model::models());
    let animating = scene::animate(list, models, &frame);
    scene::draw(list, models);
    gpu::step(frame.eye, edits);
    cloud::step(draw_list(), &frame, &ctx.pass);
    marker::step(ctx, &frame, draft);
    marker::draw(draw_list());
    if animating {
        ctx.invalidate();
    }
}
