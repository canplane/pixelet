// pxl-render/renderer.rs

//! The renderer. It draws the world's cast and the scheduler's draw lists on
//! the device, within the core's own turn. Its depth is the device's,
//! reversed as `mat4::perspective` makes it: the nearer depth is the greater.
//! A frame draws only what moved: one that reads as the last stands and
//! submits nothing, and one where only the listed draws that mark moved is
//! drawn through the gate, in its claimed tiles alone.

use pxl_content::generation::GROUPS;
use pxl_engine::eye;
use pxl_engine::light::TILE as LIGHT_TILE;
use pxl_engine::ray;
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::Mat4;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::EFFECT_REACH;
use xpute_core::abi::handle::handle_slot;
use xpute_core::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::api_spec::surface_role;
use crate::frame::shader::record::PIPELINE_SCREEN_MASK;
use crate::frame::shader::record::{
    PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR, PIPELINE_SKINNED,
};
use crate::frame::shader::sequence;
use crate::frame::shader::word::frame::{
    CAMERA_POS, CORNER_AO, DRIFT_Q, DRIFT_R, EDGE_STRENGTH, EYE, GIZMO, JITTER, LIGHT_STEPS,
    LIGHT_VIEW_PROJ, LOW_SPEC, MAP, RIM_INTENSITY, SCREEN, SHADOW_MOON, SHADOW_STRENGTH,
    SHADOW_TEXEL, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::SHADOW_SIZE;
use crate::frame::shader::word::{clouds, BOX_VERTICES, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS};
use crate::frame::shelf::Atlas;
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, device, frame_begin, frame_commit, gpu, on_device_lost,
    pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes, Device, Entry,
    Format, Gpu, Group, Image, Load, RenderContext, TimestampWrites, TrackedRenderPass,
    WORLD_COLOR,
};
use crate::gpu::draw::{drifts, uniform_of, Draw, DrawTable, Position};
use crate::gpu::draw::{
    DrawStat, DrawUpdate, DRAW_PIPELINES, DRAW_WORDS, REWRITE_WORDS, UPLOAD_WORDS,
};
use crate::gpu::generator::Mask;
use crate::gpu::image_map::ImageMap;
use crate::gpu::mip::MipPass;
use crate::gpu::pack::Pack;
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::{put_mat, put_steps};
use crate::gpu::read::{Read, SlotMirror};
use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::surface::{on_surfaces_lost, surface, surfaces, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::gpu::tile::{self, Damage, TileGrid};
use crate::gpu::upload::uploads;
use crate::look::atmosphere::{BOTTOM_RADIUS_KM, SUN_ANGULAR_RADIUS};
use crate::look::environment;
use crate::param::cloud;
use crate::pass::ao::ScreenAo;
use crate::pass::atmosphere::{Atmosphere, Upload};
use crate::pass::character::CharacterPass;
use crate::pass::cloud::CloudPass;
use crate::pass::debug::DebugPass;
use crate::pass::decor::DecorPass;
use crate::pass::finish::Finish;
use crate::pass::impostor::ImpostorPass;
use crate::pass::location::LocationPass;
use crate::pass::mark::{CopyCache, DamageInput};
use crate::pass::marker::MarkerPass;
use crate::pass::model::{frame_layout, on_skins_lost, Extent, FrameGroup, Stage};
use crate::pass::propagate::{self, Propagate};
use crate::pass::resolve::Resolve;
use crate::pass::shadow::{self, Shadow};
use crate::pass::skinned_mesh::SkinnedPass;
use crate::pass::sky::SkyPass;
use crate::pass::star::StarPass;
use crate::ray::{Cast, GroundInput, RayCount, RayPass};

const DRAWS_INIT: u32 = 256 * DRAW_FLOATS as u32;

const _: () = assert!(
    GPU_SPAN_NAMES <= 64,
    "a span a bit of the mask of the spans a frame wrote (`Gpu::submit_frame`)"
);

type Frame = [f32; FRAME_FLOATS];

/// The bits of the wind's drift in the frame words `frame` (`DRIFT_Q`), to
/// be compared bit for bit.
fn drift_bits(frame: &Frame) -> [u32; 4] {
    core::array::from_fn(|k| frame[DRIFT_Q + k].to_bits())
}

/// The wind's drift in the frame words `frame`, as a `Position` on the map's
/// plane.
fn drift_at(frame: &Frame) -> Position {
    Position {
        q: [
            frame[DRIFT_Q].to_bits() as i32,
            frame[DRIFT_Q + 1].to_bits() as i32,
            0,
        ],
        r: [frame[DRIFT_R], frame[DRIFT_R + 1], 0.0],
    }
}

pub struct FrameInput<'a> {
    /// The view, the projection from the camera to the device's clip as
    /// `mat4::perspective` makes it, and the two multiplied. All three are
    /// relative to the eye and hold no translation of it, so no large eye
    /// translation is there for a point's translation to cancel against.
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    /// The eye's position, in world doubles.
    pub eye: V3,
    /// The CSS pixels that one unit spans at one unit from the eye, as the
    /// core's screen counts them (pxl-core domain/view/frame/screen.rs
    /// `px_per_unit`). The markers are sized by it, as the core lays them out.
    pub px_per_unit: f64,
    /// How much a crease darkens (`edge_strength`), a rim brightens
    /// (`rim_intensity`) and the inside of a cube's corners darkens
    /// (`corner_ao`). Each is 0 while its setting is off.
    pub edge_strength: f64,
    pub rim_intensity: f64,
    pub corner_ao: f64,
    /// Whether the screen's occlusion is drawn (`screen_ao`), whether the
    /// faces are drawn without their textures (`low_spec`), and whether the
    /// shadows are on (`shadows`).
    pub screen_ao: bool,
    pub low_spec: bool,
    pub shadows: bool,
    /// Whether the camera gizmo is shown: the camera grid over the canvas
    /// and its tint on the rays' cells (`camera_gizmo`).
    pub camera_gizmo: bool,
    /// How much of the clouds shows, from none to all (`clouds`), and how far
    /// the wind has carried them, X and Y in units (`drift`, pxl-content
    /// cloud.rs `drift`). The core lays the clouds out by both.
    pub clouds: f64,
    pub drift: [f64; 2],
    pub update: DrawUpdate<'a>,
    pub environment: &'a [f32],
    /// Each decor group's mesh, skin image and extent. The mesh is 0 while it
    /// loads.
    pub decor: &'a [(u32, Option<Image>, Extent)],
    pub skins: &'a [(u32, Image)],
    /// Each character surface: its handle, its character's draw as the
    /// surface's camera draws it, the skin image, and the color it clears to
    /// (0xRRGGBB, sRGB). No list holds the draw (gpu/draw.rs `scaled_draw`).
    pub character_surfaces: &'a [(u32, Draw, Image, u32)],
    /// The texture pack's image; none while it loads.
    pub pack: Option<Image>,
    pub moon: Option<Image>,
    pub earth: Option<Image>,
    pub earth_layer: EarthLayer<'a>,
    /// The terrain's held tiles, whose cells the GPU draws whether or not
    /// an eye casts.
    pub ground: GroundInput<'a>,
    /// The cast of the eye's rays on the GPU, one ray a drawn pixel, into the
    /// world surface's `rays`, which the screen pass draws. It is none without
    /// an eye, and then no ray is drawn.
    pub cast: Option<Cast<'a>>,
    pub atlas: &'a mut Atlas,
    pub text: TextOf,
    /// Whether the frame's draws are counted (`FrameStat::draws`). Only the
    /// profiler sets it, so a frame the profiler does not record counts none.
    pub counted: bool,
    /// Whether what the core animates stepped this turn. The animation wakes
    /// the turn at its own steps, so the resolve asks for no frame after it.
    pub animated: bool,
}

/// How many texels beside its own a caster changes, in the light's map and,
/// through a receiver's taps, on the screen. The margin covers the map's
/// filter, a slope's push and the normal's offset, with room to spare
/// (shadow.wgsl `shadow_visibility`).
const SHADOW_MARGIN: f64 = 4.0;

/// The clip box `[x0, y0, x1, y1]`, within the screen, that the flat polygon
/// `p` covers through `m`. `p` is in units from the eye. Only the part of `p`
/// in front of the eye's plane counts, cut at that plane as Sutherland and
/// Hodgman clip. The box is none where no part of `p` is in front of the
/// plane or on the screen.
fn plane_clip(m: &Mat4, p: &[V3]) -> Option<[f32; 4]> {
    const NEAR: f64 = 1e-6;
    let to = |c: &V3| -> [f64; 4] {
        core::array::from_fn(|r| m[r] * c[0] + m[4 + r] * c[1] + m[8 + r] * c[2] + m[12 + r])
    };
    let v: Vec<[f64; 4]> = p.iter().map(to).collect();
    let mut b: Option<[f64; 4]> = None;
    let mut take = |c: [f64; 4]| {
        let (x, y) = (c[0] / c[3], c[1] / c[3]);
        let e = b.get_or_insert([x, y, x, y]);
        *e = [e[0].min(x), e[1].min(y), e[2].max(x), e[3].max(y)];
    };
    for k in 0..v.len() {
        let (a, c) = (v[k], v[(k + 1) % v.len()]);
        if a[3] > NEAR {
            take(a);
        }
        if (a[3] > NEAR) != (c[3] > NEAR) {
            let t = (NEAR - a[3]) / (c[3] - a[3]);
            take(core::array::from_fn(|i| a[i] + (c[i] - a[i]) * t));
        }
    }
    let b = b?;
    if b[2] < -1.0 || b[0] > 1.0 || b[3] < -1.0 || b[1] > 1.0 {
        return None;
    }
    Some(b.map(|v| v.clamp(-1.0, 1.0) as f32))
}

/// The least height of a light, as the sine of its angle, at which it still
/// reaches the land under an eye `alt_km` up. That height lies below the
/// horizon by the horizon's dip at that altitude and by the light's disc
/// radius, where the air's light ends (atmos_lut.wgsl `sun_transmittance`).
fn least_height(alt_km: f64) -> f64 {
    let r = BOTTOM_RADIUS_KM;
    -libm::sin(libm::acos(r / (r + alt_km.max(0.0))) + SUN_ANGULAR_RADIUS)
}

/// The pipelines of the world's draws whose moves draw the frame whole
/// (`beside`).
const BESIDE: [u32; 4] = [
    PIPELINE_DECOR,
    PIPELINE_GHOST_BOX,
    PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR,
];

/// Whether the draw `d`, with uniform `u`, is one whose moves draw the frame
/// whole. Any draw of `BESIDE`'s pipelines is, except a cloud's. A cloud
/// drifts with the wind, and the marks of the decor and of the impostors
/// carry it where it was and where it is, each at its frame's drift
/// (pass/mark.rs).
fn beside(d: &[u32; DRAW_WORDS], u: &[f32]) -> bool {
    BESIDE.contains(&d[4]) && !drifts(u)
}

/// Whether the turn moved a draw whose moves draw the frame whole
/// (`beside`). It did if it changed one, as it was or as it is, or if an
/// upload one reads moved (`touched`, sorted).
fn beside_changed(update: &DrawUpdate, touched: &[u32]) -> bool {
    let draws = update.draws;
    let changed = update.changes.iter().any(|c| {
        draws.now(c.slot).is_some_and(|(d, u)| beside(&d, &u))
            || c.was.is_some_and(|(d, u)| beside(&d, &u))
    });
    let read_changed = || {
        BESIDE.iter().any(|&p| {
            draws.of(p).any(|d| {
                beside(d, uniform_of(draws.uniforms, d)) && touched.binary_search(&d[0]).is_ok()
            })
        })
    };
    changed || (!touched.is_empty() && read_changed())
}

/// What a frame read that decides what it draws (`FramePlan`). `partial` says
/// that nothing but the listed draws that mark moved before its cast.
/// `still` says that all it reads read as last time except its lists and its
/// jitter, over rays cast again at most at their own pixels. `lists_same`
/// says that nothing in the lists changed this turn.
/// `clouds_damage` says that the cloud sheets in view moved. The rest are the
/// rays' own: whether the light's receivers are marked afresh, which casts
/// every pixel (`unmasked`, `RayPass::unmasked`), whether the rays cast pixels
/// (`casts_px`) and whether the light casts (`light_casts`).
#[derive(Clone, Copy, Debug, Default)]
struct FrameRead {
    partial: bool,
    still: bool,
    lists_same: bool,
    clouds_damage: bool,
    unmasked: bool,
    casts_px: bool,
    light_casts: bool,
}

/// What a frame draws. It is a pure function of what the frame read
/// (`FrameRead`), and renderer.test.rs explores it whole. `quiet` holds where
/// nothing moved but the jitter. `masked` holds where only the listed draws
/// that mark moved: the frame draws again only its claimed tiles. Otherwise
/// the frame is drawn whole and its pixels are cast whole, as they also are
/// where the light's receivers are marked afresh off every pixel's walk.
/// `unchanged` holds where nothing moved, not even the jitter, and nothing is
/// cast. Then nothing is drawn or submitted, and the canvases keep the frame
/// they show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FramePlan {
    quiet: bool,
    masked: bool,
    unchanged: bool,
}

impl FramePlan {
    fn of(r: FrameRead) -> FramePlan {
        let quiet = r.still && r.lists_same && !r.clouds_damage;
        FramePlan {
            quiet,
            masked: r.partial && r.still && !r.unmasked,
            unchanged: quiet && !r.casts_px && !r.light_casts,
        }
    }
}

/// The earth layer's tiles as the GPU reads them. `window` is the floor's
/// window, whose south-west tile comes first, and `layers` holds each tile's
/// layer plus one. `upload` hands the writer each tile that has come, while
/// the writer takes it.
pub struct EarthLayer<'a> {
    pub window: [i32; 2],
    pub layers: &'a [u32],
    pub upload: &'a mut Upload,
}

pub struct FrameOutput {
    /// Which case the frame turn ended in: no device or surface, not
    /// admitted, dropped from the stream, or the kind of frame it drew.
    pub kind: FrameKind,
    /// Whether another turn is wanted, as when a surface waits for its size
    /// to settle, a frame was dropped, or the resolve asks for more frames.
    pub redraw: bool,
    pub stat: Option<FrameStat>,
}

/// A GPU read as it lands. `frame` is the number of the frame it timed
/// (`FrameStat::frame` as it was then), and `spans` holds each span's time.
/// `busy_ms` is the time the spans ran, with spans that overlap counted once
/// (`Gpu::gpu_busy_ms`). `px` is the world's pixels that frame drew, which
/// the spans were measured over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuRead {
    pub frame: f64,
    pub spans: [f64; GPU_SPAN_NAMES],
    pub busy_ms: f64,
    pub px: f64,
}

impl GpuRead {
    /// The read's time in the spans that run threads over the world's drawn
    /// pixels (span.json `per_px`, `GpuSpanName::PER_PX`). It is the time a
    /// lower drawn resolution can buy back.
    pub fn px_ms(&self) -> f64 {
        GpuSpanName::PER_PX
            .iter()
            .map(|&s| self.spans[s as usize])
            .sum()
    }
}

#[derive(Clone, Copy)]
pub struct FrameStat {
    /// The GPU read that landed with this frame, or none.
    pub gpu: Option<GpuRead>,
    /// This frame's number, counting from 1 every frame the renderer encoded,
    /// or 0 where the frame stood and drew nothing.
    pub frame: f64,
    /// The count of the upload table's uploads (`uploads`), their bytes
    /// (`upload_bytes`) and the texture pack's bytes (`pack_bytes`).
    pub uploads: f64,
    pub upload_bytes: f64,
    pub pack_bytes: f64,
    /// The draws and the triangles of each pipeline, counted only while
    /// `FrameInput::counted`. Their sums are the frame's draws and triangles.
    pub draw_stat: DrawStat,
    /// What changed that cast the light's map this frame, a bit each in
    /// `LIGHT_CHANGED`'s order (ray/cast.rs `light_changed`), or 0 where it
    /// cast none. `decor_rects` counts the rects that marked decor's slots to
    /// generate again (gpu/generator.rs `marked_rects`), or 0 where none did.
    pub light_changed: f64,
    pub decor_rects: f64,
    /// The ray counts of the eye's cast and of the light's from an earlier
    /// frame, which landed with this one, named by that frame's number
    /// (ray/count.rs `RayCount`).
    pub ray_cnt: Option<RayCount>,
}

static RENDERER: Global<Option<Renderer>> = Global::new();

/// The renderer's pipelines while their shader modules are made, a few a
/// turn, before the renderer exists (`render_frame`). The renderer takes them
/// once all the modules are made.
static MAKING: Global<Option<PipelineCache>> = Global::new();

/// How much of what the renderer makes before its first frame is made, from
/// 0 to 1 (`PipelineCache::share`). The UI's progress bar shows it. It is 0 with
/// no device and 1 once the renderer draws.
pub fn renderer_progress() -> f64 {
    if let Some(r) = RENDERER.get(|| None).as_ref() {
        return r.pipelines.share();
    }
    MAKING
        .get(|| None)
        .as_ref()
        .map_or(0.0, PipelineCache::share)
}

/// Each light's step, the sun's first: how far its height moves before the
/// sky as drawn moves by a display level or, while it is out of reach,
/// before it comes into reach (pass/atmosphere.rs `steps`). A step is 0, the
/// least, before any has landed. Without a renderer no sky is drawn, and
/// both steps are infinite.
pub fn sky_steps() -> [f32; 2] {
    RENDERER
        .get(|| None)
        .as_ref()
        .map_or([f32::INFINITY; 2], |r| r.atmosphere.steps())
}

static REDRAW: Global<bool> = Global::new();

/// The count of frames encoded since boot, drawn or dropped, which is the
/// last one's number (`FrameStat::frame`). It lives beside the renderer, so
/// the renderer made after a device is lost numbers on from it.
static DRAWN_FRAMES: Global<f64> = Global::new();

/// The number the frame being made takes when it is encoded. A pass that
/// counts names its counts by it (ray/count.rs `Frame`). A frame that
/// stands takes no number, and nothing counts in it.
fn next_frame() -> f64 {
    *DRAWN_FRAMES.get(|| 0.0) + 1.0
}

pub fn on_gpu_lost() {
    RENDERER.set(None);
    MAKING.set(None);
    on_skins_lost();
    on_device_lost();
    uploads().lose();
    on_surfaces_lost();
}

pub fn on_surface_closed(s: Surface) {
    for id in s.targets() {
        release(id);
    }
    let slot = handle_slot(s.handle) as usize;
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        if let Some(pass) = r.character_passes[slot].take() {
            pass.release();
        }
    }
}

pub struct Renderer {
    align: u32,
    /// The spans' query set (`Gpu::spans`), or 0 where the device keeps no
    /// timestamps. Then no frame is timed.
    query_set: u32,
    /// For each read on its way back (`Gpu::submit_frame`), the world's pixels
    /// and the number of the frame whose spans it timed.
    read_px: [f64; api::READS],
    read_frame: [f64; api::READS],
    pipelines: PipelineCache,
    frame_buf: u32,
    frame: Frame,
    sky: SkyPass,
    ghost: Pipeline,
    draws_buf: u32,
    draws_bytes: u32,
    bind_group: u32,
    model_group: FrameGroup,
    shadow: Shadow,
    /// Whether the shadow map is cast and drawn this frame. It is while the
    /// map is made (`Shadow::on`) and the sun or the moon is up (`prepare`).
    shadows: bool,
    /// Whether the debug pass drew into the screen's depth last frame. If it
    /// did, the finish carries the depth whole again (pass/finish.rs).
    debug_drew: bool,
    ao: ScreenAo,
    screen_ao: bool,
    atmosphere: Atmosphere,
    star: StarPass,
    cloud: CloudPass,
    mip: MipPass,
    pack: Pack,
    moon: ImageMap,
    decor: DecorPass,
    impostor: ImpostorPass,
    skinned_mesh: SkinnedPass,
    location: LocationPass,
    marker: MarkerPass,
    finish: Finish,
    resolve: Resolve,
    /// Carries the light map's tiles that changed over to the screen's tiles
    /// whose pixels read them.
    propagate: Propagate,
    debug: DebugPass,
    screen: RayPass,
    /// The frame's input as the last frame drew it (stage 2, `frame`): the
    /// frame words except the jitter, the sky's, the stars', the occlusion's
    /// and the air's words, the world surface's targets, the skins and the
    /// character surfaces. It also marks what landed or was drawn again
    /// without words, such as an image or the impostors' bake. The cloud
    /// sheets are read apart (`clouds_read`).
    frame_read: Read,
    /// The jitter and the drift in the frame words as the GPU's copy holds
    /// them (`JITTER`, `DRIFT_Q`).
    jitter_written: [f32; 2],
    drift_written: [u32; 4],
    /// The cloud sheets' words as last written (`CloudPass::words`).
    clouds_read: Read,
    /// The gate that a frame not drawn whole is drawn through, and its group
    /// over the tiles' words and uniform (`Resolve::tiles`).
    mask: Pipeline,
    mask_group: Group,
    /// The marks' groups 0 (tile_mark.wgsl, `mark_entries`). `marks` holds
    /// the screen's, over this frame's words and over the last frame's.
    /// `light_mark` is the light map's, over the light's frame.
    marks: [Group; 2],
    light_mark: Group,
    /// The light map's tiles, each as wide as a tile of the light's cast
    /// (pxl-engine `light::TILE`). A tile is claimed only in the frame it
    /// changed in, since the map gathers nothing over frames (gpu/tile.rs).
    light_tiles: TileGrid,
    /// The uploads that the marking draws read, as the last frame drew them
    /// (pass/mark.rs).
    last_frame_uploads: CopyCache,
    /// The frame words the last drawn frame drew with, and their buffer,
    /// which is written when the screen's marks draw. A draw as it was is
    /// marked by these words, so its mark has the jitter and the wind's drift
    /// the draw was drawn at (pass/mark.rs).
    last_frame_words: Frame,
    last_frame_buf: u32,
    /// What the occlusion was last found from: the frame words, the
    /// occlusion's own words, the surface's size, whether decor is generated,
    /// and the world's depth that the static opaque draws lay. The frame is
    /// drawn again for more than these, but the occlusion is found again only
    /// when they moved.
    ao_read: Read,
    character_passes: [Option<CharacterPass>; SURFACES],
    /// Whether the device holds the draws' uniforms as the lists left them
    /// (`draws_buf`).
    uniforms_mirror: SlotMirror,
    /// A frame's lookup tables, kept from frame to frame for their room: the
    /// handles its lists touch (`touched`) and the uploads its marks read
    /// (`read_uploads`).
    touched: Vec<u32>,
    read_uploads: Vec<u32>,
}

impl Renderer {
    /// Makes the renderer over `pipelines`, once every shader's module is made
    /// (`PipelineCache::make_modules`). Each of its pipelines is asked for here,
    /// the cast's and the ground's among them (`RayPass::make`), so none is
    /// asked for once it draws.
    fn new(g: &Gpu, device: Device, mut pipelines: PipelineCache) -> Renderer {
        let frame_buf = g.buf(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buf);
        let atmosphere = Atmosphere::new(g, &mut pipelines);
        let air = atmosphere.entries();
        let star = StarPass::new(g, &mut pipelines, frame_buf, air);
        let cloud = CloudPass::new(g, &mut pipelines, frame_buf, air);
        let mip = MipPass::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let pack = Pack::new(g);
        let moon = ImageMap::new(g, MOON_TEXELS, MOON_TEXELS);
        let sky = SkyPass::new(g, &mut pipelines, frame_buf, &moon, &atmosphere);
        let sky_buf = sky.uniform();
        let draws_buf = g.buf(
            DRAWS_INIT * 4,
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let shadow = Shadow::new(g, &mut pipelines);
        let model_group = FrameGroup::new(
            g,
            frame_layout(g, &mut pipelines),
            frame_buf,
            shadow.entries(1),
            air,
        );
        let query_set = if device.timestamps { g.spans() } else { 0 };
        let mut r = Renderer {
            align: device.align,
            query_set,
            read_px: [0.0; api::READS],
            read_frame: [0.0; api::READS],
            ghost: pipelines.build_world(g, PIPELINE_GHOST_BOX),
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned_mesh: SkinnedPass::new(g, &mut pipelines, device.align),
            location: LocationPass::new(g, &mut pipelines, frame_buf, device.align),
            marker: MarkerPass::new(g, &mut pipelines, frame_buf, device.align),
            finish: Finish::new(g, &mut pipelines),
            resolve: Resolve::new(g, &mut pipelines),
            propagate: Propagate::new(g, &mut pipelines),
            debug: DebugPass::new(g, &mut pipelines, frame_buf, sky_buf, device.align),
            screen: RayPass::new(g, &mut pipelines),
            frame_read: Read::default(),
            jitter_written: [f32::NAN; 2],
            drift_written: [u32::MAX; 4],
            clouds_read: Read::default(),
            mask: pipelines.build_mask(g, PIPELINE_SCREEN_MASK),
            mask_group: Group::default(),
            marks: [Group::default(), Group::default()],
            light_mark: Group::default(),
            light_tiles: TileGrid::new(
                g,
                &mut pipelines,
                LIGHT_TILE,
                1,
                GpuSpanName::shadow_damage,
            ),
            last_frame_uploads: CopyCache::default(),
            last_frame_words: [0.0; FRAME_FLOATS],
            last_frame_buf: g.buf(
                word_bytes(FRAME_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            ao_read: Read::default(),
            pipelines,
            frame_buf,
            frame: [0.0; FRAME_FLOATS],
            sky,
            draws_buf,
            draws_bytes: DRAWS_INIT * 4,
            bind_group: 0,
            model_group,
            shadow,
            shadows: false,
            debug_drew: false,
            ao,
            screen_ao: false,
            atmosphere,
            star,
            cloud,
            mip,
            pack,
            moon,
            character_passes: [const { None }; SURFACES],
            uniforms_mirror: SlotMirror::default(),
            touched: Vec::new(),
            read_uploads: Vec::new(),
        };
        r.screen.make(g, &mut r.pipelines);
        r.box_groups(g);
        r
    }

    /// Whether every pipeline the renderer asked for is made, so that it may
    /// draw. Once they are, it bakes the cloud field (`CloudPass::bake`).
    fn ready(&mut self, g: &Gpu) -> bool {
        if !self.pipelines.poll() {
            return false;
        }
        self.cloud.bake(g);
        true
    }

    /// Sets the groups the frame's passes keep across frames (`Group`). The
    /// caller sets them before the frame's commands, after the world's
    /// surface `s` is made at its size (`targets`) and the screen's and the
    /// map's tiles at theirs (`TileGrid::size`). Each group is set over those
    /// and over whatever else it binds.
    fn set_groups(&mut self, g: &Gpu, s: &Surface) {
        let tiles = &self.resolve.tiles;
        let layout = self.skinned_mesh.mark_layout();
        let frames = [self.frame_buf, self.last_frame_buf];
        for (group, frame) in self.marks.iter_mut().zip(frames) {
            group.set(g, layout, &mark_entries(frame, tiles));
        }
        if self.shadow.on() {
            let (light_frame, _) = self.shadow.frame_and_map();
            let entries = mark_entries(light_frame, &self.light_tiles);
            self.light_mark.set(g, layout, &entries);
        }
        let frame_tiles = tiles.this_frame();
        let entries = [buf_entry(30, frame_tiles.0), buf_entry(31, frame_tiles.1)];
        self.mask_group.set(g, self.mask.layouts[0], &entries);
        if self.screen_ao {
            self.ao.set_apply_group(g, frame_tiles);
        }
        let reads = self.screen.light_read().map(|r| r.list);
        self.shadow.set_groups(g, &self.light_tiles, reads);
        if self.shadows {
            let depth = s.depth_only_view;
            self.propagate
                .set_groups(g, depth, &self.light_tiles, &self.resolve.tiles);
        }
        self.resolve.set_groups(g, self.frame_buf, s);
        let tiles = Some(&self.resolve.tiles);
        self.finish.set_groups(g, self.frame_buf, s, tiles);
    }

    /// The entries a cell's face is painted by, wherever it is drawn from
    /// (paint.wgsl): the frame words `frame`, the light's shadow map, the
    /// texture pack and the air.
    fn lit_entries(&self, frame: u32) -> Vec<Entry> {
        let [map, sampler] = self.shadow.entries(1);
        let mut e = vec![
            buf_entry(0, frame),
            map,
            sampler,
            view_entry(4, self.pack.view),
            buf_entry(5, self.pack.table),
            sampler_entry(6, self.pack.sampler),
        ];
        e.extend(self.atmosphere.entries());
        e
    }

    /// Makes what only the shadows hold while their setting `on` is on
    /// (`FrameInput::shadows`), and lets it go while it is off, before the
    /// frame. The shadows hold the map and the world's depth seen from the
    /// light (`Shadow::made`), and the light's cast (`RayPass::light_made`).
    /// When the setting turned, the lit passes' groups are made again, over
    /// the map or over the blank one.
    fn shadows_made(&mut self, g: &Gpu, on: bool) {
        self.screen.light_made(g, on);
        if self.shadow.sync(g, on) {
            self.model_group.release();
            self.model_group = FrameGroup::new(
                g,
                frame_layout(g, &mut self.pipelines),
                self.frame_buf,
                self.shadow.entries(1),
                self.atmosphere.entries(),
            );
            self.box_groups(g);
        }
    }

    /// Makes the ghost box's group 0 over what a cell's face is painted by
    /// and the draws' uniforms. It also hands the draws' buffer to the skinned
    /// and the location passes, and the lit entries to the screen pass.
    fn box_groups(&mut self, g: &Gpu) {
        release(self.bind_group);
        let mut e = self.lit_entries(self.frame_buf);
        e.push(buf_entry(3, self.draws_buf));
        self.bind_group = g.bind_group(self.ghost.layouts[0], &e);
        self.skinned_mesh.set_draws(g, self.draws_buf);
        self.location.set_draws(g, self.draws_buf);
        let lit = self.lit_entries(self.frame_buf);
        self.screen.set_lit(lit);
    }

    /// Puts the frame words from `input`, over the world's surface `world`
    /// made at its size, and from `cast`, the eye the rays leave from.
    /// Without a cast, the eye's origin and reach are zero.
    fn prepare(&mut self, g: &Gpu, world: &Surface, input: &FrameInput, cast: Option<&Cast>) {
        let (width, height, env) = (world.width, world.height, input.environment);
        // How far the eye draws, in units. The clouds are drawn as far.
        let eye_far = cast.map_or(0.0, |c| f32::from_bits(c.eye[eye::FAR]));
        // The markers measure their size in view space.
        let b = &mut self.marker;
        put_mat(&mut b.view, 0, &input.view);
        put_mat(&mut b.proj, 0, &input.proj);
        b.px_per_unit = input.px_per_unit as f32;

        let u = &mut self.frame;
        let m = input.view_proj;
        put_mat(u, VIEW_PROJ, &m);
        // Everything is drawn in the eye's frame. On the GPU a draw's
        // position is taken against the eye's Q3 (gpu/draw.rs `Position`),
        // which is the Q3 the rays leave from (`eye::ORIGIN`). What is
        // rasterized then stands about the same eye the world is drawn from.
        // Without a cast no world is drawn, and the eye's Q3 is zero. The eye
        // is held on Q3 steps (pxl-core domain/view/frame/pose.rs
        // `view_compose`), so its Q3 is exact.
        let origin = cast.map_or([0.0; 3], |c| {
            core::array::from_fn(|k| f32::from_bits(c.eye[eye::ORIGIN + k]))
        });
        u[EYE..EYE + 3].copy_from_slice(&origin);
        self.shadow.put_eye(origin);
        let eye_world = input.eye;
        let back = Position::of([input.drift[0], input.drift[1], 0.0]);
        u[DRIFT_Q] = f32::from_bits(back.q[0] as u32);
        u[DRIFT_Q + 1] = f32::from_bits(back.q[1] as u32);
        u[DRIFT_R..DRIFT_R + 2].copy_from_slice(&back.r[..2]);
        self.shadow.put_drift(&u[DRIFT_Q..DRIFT_Q + 4]);
        self.resolve.prepare(&m, eye_world);
        let eye = [0.0; 3];
        self.sky.prepare(&m, env, self.moon.loaded);
        self.cloud.prepare(
            [eye_world[0], eye_world[1]],
            input.drift,
            input.clouds,
            eye_far * env[environment::AIR + 1],
        );
        self.star.prepare(&m, env, world.px_per_css());
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
        }
        u[EDGE_STRENGTH] = input.edge_strength as f32;
        u[LOW_SPEC] = if input.low_spec { 1.0 } else { 0.0 };
        u[GIZMO] = if input.camera_gizmo { 1.0 } else { 0.0 };
        u[RIM_INTENSITY] = input.rim_intensity as f32;
        u[CORNER_AO] = input.corner_ao as f32;
        u[VIEWPORT] = width as f32;
        u[VIEWPORT + 1] = height as f32;
        u[SCREEN] = world.screen.0 as f32;
        u[SCREEN + 1] = world.screen.1 as f32;
        u[VIGNETTE] = 1.0;
        self.screen_ao = input.screen_ao;
        if self.screen_ao {
            self.ao.prepare(&input.proj, width, height);
        }
        // The eye's place on the map: its Mercator longitude and ordinate,
        // each its units over the map's radius in units, and that radius.
        let lon = world::nearest(eye_world[0], 0.0) / MERCATOR_RADIUS_U;
        u[MAP..MAP + 3].copy_from_slice(&[
            lon as f32,
            (eye_world[1] / MERCATOR_RADIUS_U) as f32,
            MERCATOR_RADIUS_U as f32,
        ]);
        // The shadow map is as wide as the effects reach. It is cast from the
        // sun while the sun still reaches the land, and otherwise from the
        // moon while the moon does. There is none while the shadows are off
        // or while neither light reaches the land: only the lights' direct
        // light reads the map (atmos_light.wgsl `surface_light_in`), so a
        // shadow would show nowhere.
        let (sun, moon) = (
            &env[environment::SUN..environment::SUN + 3],
            &env[environment::MOON..environment::MOON + 3],
        );
        let low = least_height(f64::from(env[environment::AIR]));
        let light = if f64::from(sun[2]) > low {
            Some((sun, false))
        } else if f64::from(moon[2]) > low {
            Some((moon, true))
        } else {
            None
        };
        let light = light.filter(|_| self.shadow.on());
        self.shadows = light.is_some();
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_MOON] = if light.is_some_and(|l| l.1) { 1.0 } else { 0.0 };
        if let Some((dir, _)) = light {
            // A receiver reads the map within the effects' reach.
            let half_width = shadow::half_width(EFFECT_REACH);
            let reach_ndc = (EFFECT_REACH / half_width) as f32;
            self.shadow
                .aim(dir, eye, half_width, reach_ndc, eye_world.map(|v| -v));
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
            put_steps(u, LIGHT_STEPS, &self.shadow.light_view_proj);
            u[SHADOW_TEXEL] = self.shadow.texel as f32;
        }

        self.atmosphere.prepare(g, env, eye_far);
    }

    /// Draws the ghost box, the cube box.wgsl unfolds from its draw's uniform. The draw's
    /// uniform slot rides on the first vertex, `uniform * BOX_VERTICES`, and box.wgsl divides
    /// the vertex index by `BOX_VERTICES` to find it.
    fn draw_ghost_box(&self, pass: &mut TrackedRenderPass, draws: &DrawTable) {
        for d in draws.of(PIPELINE_GHOST_BOX) {
            pass.set_pipeline(self.ghost.id);
            pass.set_bind_group(0, self.bind_group, &[]);
            pass.draw(BOX_VERTICES, d[1], d[3] * BOX_VERTICES, 0);
        }
    }

    /// The drawn pixels the cloud sheets cover, which mark the screen's tiles,
    /// or none where no sheet is seen. Each sheet is laid as cloud.wgsl `vs`
    /// lays it: a square at its height over the eye, as wide as the eye draws
    /// clouds, cut at the eye's plane.
    fn sheets_damage(
        &self,
        view_proj: &Mat4,
        env: &[f32],
        width: u32,
        height: u32,
    ) -> Option<Damage> {
        let w = self.cloud.words();
        if w[clouds::SHOWN] <= 0.0 {
            return None;
        }
        let (alt, km_per_u) = (
            f64::from(env[environment::AIR]),
            f64::from(env[environment::AIR + 1]),
        );
        let dist = f64::from(w[clouds::DRAW_DIST]);
        let mut clip: Option<[f32; 4]> = None;
        for km in [cloud::altocumulus::KM, cloud::cirrus::KM] {
            let h = f64::from(km) - alt;
            let half = libm::sqrt((dist * dist - h * h).max(0.0)) / km_per_u.max(f64::MIN_POSITIVE);
            if half <= 0.0 {
                continue;
            }
            let z = h / km_per_u.max(f64::MIN_POSITIVE);
            let quad = [
                [-half, -half, z],
                [half, -half, z],
                [half, half, z],
                [-half, half, z],
            ];
            if let Some(b) = plane_clip(view_proj, &quad) {
                clip = Some(clip.map_or(b, |c| {
                    [
                        c[0].min(b[0]),
                        c[1].min(b[1]),
                        c[2].max(b[2]),
                        c[3].max(b[3]),
                    ]
                }));
            }
        }
        clip.and_then(|c| tile::damage_of(c, width, height, tile::DAMAGE_MARGIN))
    }

    /// Puts into `out` the uploads that the listed marking draws read, each
    /// once, for `CopyCache` to keep (pass/mark.rs).
    fn marks_read(&self, draws: &DrawTable, out: &mut Vec<u32>) {
        out.clear();
        self.skinned_mesh
            .marks_read(draws.of(PIPELINE_SKINNED), out);
        self.decor.marks_read(draws, out);
        self.impostor.marks_read(draws.of(PIPELINE_IMPOSTOR), out);
        self.location.marks_read(draws, out);
        out.sort_unstable();
        out.dedup();
    }

    /// Called when a turn takes the lists and draws no frame. The next frame
    /// does not see the uploads and the draws they moved, so it is drawn
    /// whole, the draws' uniforms are written whole, and what the marks read
    /// is copied again (`CopyCache::forget`).
    fn put_off(&mut self) {
        self.frame_read.forget();
        self.uniforms_mirror.forget();
        self.last_frame_uploads.forget();
    }

    /// Draws a frame in two stages. Each stage stands, and makes nothing new,
    /// while what it reads reads the same. Stage 1 is the cast's rays
    /// (`RayPass::prepare`). Stage 2 is the frame (`frame_read`): everything
    /// the world and its light make from those rays, and over it what moves
    /// on its own. That is the characters, whose clips play, the clouds, which
    /// the wind carries, the location indicator, the ghosts, the markers and
    /// the gizmos. The rays hold the world as the cast left it, so the frame
    /// can draw the world again from them without casting.
    fn frame(
        &mut self,
        g: &Gpu,
        world: &mut Surface,
        mut input: FrameInput,
    ) -> (FrameKind, FrameStat) {
        let mut read = core::mem::take(&mut self.frame_read);
        read.begin();
        self.decor.set_models(input.decor);
        self.impostor.set_models(
            input
                .decor
                .iter()
                .map(|&(mesh, image, extent)| (mesh, image.map(|i| i.0), extent)),
        );
        self.skinned_mesh.set_skins(input.skins);
        read.read(input.skins);
        read.mark(self.pack.step(g, &self.mip, input.pack));
        read.mark(self.moon.step(g, &self.mip, input.moon));
        read.mark(self.atmosphere.earth.step(g, &self.mip, input.earth));
        let mut cast = input.cast.take();
        // What the frame cannot put off is recorded before what can wait.
        // The light's cast is made first, then the bricks are uploaded, and
        // their uploads take only the room left in the stream.
        self.shadows_made(g, input.shadows);
        self.screen.hand(g, &mut input.ground);
        let layer = &mut input.earth_layer;
        read.mark(
            self.atmosphere
                .set_earth_tiles(g, layer.window, layer.layers, layer.upload),
        );
        read.mark(
            self.marker
                .sync_atlas(g, &self.mip, input.atlas, input.text),
        );
        let update = &input.update;
        // The handles this frame's lists upload, rewrite or free, sorted to
        // be looked up.
        let mut touched = core::mem::take(&mut self.touched);
        touched.clear();
        touched.extend(
            update
                .uploads
                .as_chunks::<UPLOAD_WORDS>()
                .0
                .iter()
                .map(|u| u[0]),
        );
        touched.extend(
            update
                .rewrites
                .as_chunks::<REWRITE_WORDS>()
                .0
                .iter()
                .map(|r| r[0]),
        );
        touched.extend_from_slice(update.frees);
        touched.sort_unstable();
        touched.dedup();
        let uploads_changed = take_lists(Some(g), update);
        let draws = update.draws;
        let uniforms = draws.uniforms;
        // The draws' uniforms go in a buffer with room for them all. Only the
        // slots the turn changed are written, one record for each run of
        // adjacent slots (`SlotMirror::write`).
        let uniform_bytes = word_bytes(uniforms.len());
        let grew = uniform_bytes > self.draws_bytes;
        if grew {
            release(self.draws_buf);
            self.draws_bytes = uniform_bytes * 2;
            self.draws_buf = g.buf(
                self.draws_bytes,
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            self.box_groups(g);
            self.uniforms_mirror.forget();
        }
        let uniforms_changed = self.uniforms_mirror.write(
            g,
            self.draws_buf,
            bytes(uniforms),
            word_bytes(DRAW_FLOATS) as usize,
            update.changes.iter().map(|c| c.slot),
        );
        // What the passes made of the draws stands while the turn changed
        // none, drew none in another order, and the device already held their
        // uniforms.
        let lists_changed = uniforms_changed || !update.changes.is_empty() || update.reordered;
        read.read(input.character_surfaces);

        targets(g, world);
        let animated = input.animated;
        self.prepare(g, world, &input, cast.as_ref());
        // The cast takes the light's box as the map is aimed, or zeros where
        // no shadow is cast.
        if let Some(c) = cast.as_mut() {
            let sun = if self.shadows {
                self.shadow.light_words
            } else {
                [0; eye::LIGHT_WORDS]
            };
            c.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&sun);
        }
        // The jitter: where each pixel's ray passes through it this frame,
        // the same for the cast and for what is drawn over the world
        // (pass/resolve.rs). The frame words hold it in clip units.
        let j = self.resolve.jitter();
        if let Some(c) = cast.as_mut() {
            c.eye[eye::JITTER] = j[0].to_bits();
            c.eye[eye::JITTER + 1] = j[1].to_bits();
        }
        self.frame[JITTER] = 2.0 * j[0] / world.width as f32;
        self.frame[JITTER + 1] = -2.0 * j[1] / world.height as f32;
        // Where the wind moved the cloud sheets, the screen's tiles they are
        // seen in are marked, as the draws that changed mark theirs
        // (`MarkPass::prepare`).
        let clouds_changed = {
            let r = &mut self.clouds_read;
            r.begin();
            r.read(self.cloud.words());
            !r.end()
        };
        let sheets = clouds_changed
            .then(|| {
                self.sheets_damage(
                    &input.view_proj,
                    input.environment,
                    world.width,
                    world.height,
                )
            })
            .flatten();
        // Sheets seen moving draw a frame, however still the rest is.
        let clouds_damage = sheets.is_some();
        let ao_remade = self.screen_ao
            && self
                .ao
                .ensure(g, world.width, world.height, world.depth_only_view);
        if ao_remade {
            read.mark(true);
        }
        // The jitter and the drift are read apart from the other words. Where
        // only the jitter moved, the frame draws its pixels again and stands
        // once the jitter holds (pass/resolve.rs). Where the drift moved, the
        // clouds mark their tiles.
        let jitter = [self.frame[JITTER], self.frame[JITTER + 1]];
        let drift_now = drift_bits(&self.frame);
        let drift_changed = drift_now != self.drift_written;
        let words_changed = read.read(&self.frame[..JITTER]);
        if words_changed || jitter != self.jitter_written || drift_changed {
            g.write_buf(self.frame_buf, 0, bytes(&self.frame));
            self.jitter_written = jitter;
            self.drift_written = drift_now;
        }
        // Whether nothing in the lists changed this turn.
        let drifted = draws.drifting() && drift_now != drift_bits(&self.last_frame_words);
        let lists_same = !lists_changed && !uploads_changed && !drifted;
        if words_changed || drift_changed {
            // The light's frame words are made of the same words, so they are
            // written again too.
            self.shadow.write(g);
        }
        if read.read(self.sky.words()) {
            self.sky.write(g);
        }
        if read.read(self.star.words()) {
            self.star.write(g);
        }
        if self.screen_ao && read.read(self.ao.words()) {
            self.ao.write(g);
        }
        if read.read(self.atmosphere.words()) {
            self.atmosphere.write(g);
        }
        read.read(&[world.depth, world.rays, u32::from(self.screen_ao)]);
        let b = uploads();
        self.decor.prepare(g, &draws, uniforms, lists_changed, b);
        self.impostor
            .prepare(g, draws.of(PIPELINE_IMPOSTOR), uniforms, lists_changed, b);
        self.skinned_mesh.prepare(g, draws.of(PIPELINE_SKINNED), b);
        self.location.prepare(g, &draws, uniforms, lists_changed, b);
        self.marker
            .prepare(g, world.screen_depth_view, world.screen, &draws, b);
        self.debug.prepare(g, &draws, uniforms, lists_changed);
        let meshes: [u32; GROUPS as usize] =
            core::array::from_fn(|k| self.decor.model_of(g, k as u32, b).map_or(0, |m| m.2));
        // The frame is drawn through the gate where nothing but the listed
        // draws that mark moved so far. What moved marks its tiles, and the
        // tiles still gathering since they last changed stay claimed. The
        // cast takes the claimed tiles and casts its pixels there alone.
        let partial = read.same() && !beside_changed(update, &touched);
        // The tiles are sized before anything binds them. The pixels' cast
        // casts the claimed tiles, its samples' raster draws them, and the
        // rays' screen pass draws them. The depth's bounds are sized with
        // them; the world's pass sets those bounds where the shadows are cast
        // (`Propagate::set_bounds`).
        let size = [world.width, world.height];
        self.resolve.tiles.size(g, size);
        if self.shadows {
            self.propagate.size(g, size);
        }
        let frame_tiles = self.resolve.tiles.this_frame();
        let [across, down] = self.resolve.tiles.squares();
        self.screen.set_tiles(frame_tiles, across * down);
        // Reads back the counts that earlier frames copied out.
        self.screen.cnts_back(g);
        self.screen.prepare(
            g,
            cast,
            &meshes,
            (world.rays_view, [world.width, world.height]),
            (
                self.shadow.world(),
                self.shadow.generation,
                self.shadow.starts(),
            ),
            partial,
            next_frame(),
        );
        // The frame draws the generated decor, so it reads whether decor is
        // generated. Decor turned off generates nothing and casts no ray
        // again, so without this read the frame would stand with the decor
        // still shown.
        read.read(&[u32::from(self.screen.generated().is_some())]);
        self.prepare_character_surfaces(g, input.character_surfaces);
        let q = self.query_set;
        // A query that no pass wrote holds an earlier frame's value, so the
        // frame passes along which queries were written.
        let mut stamps: [TimestampWrites; GPU_SPAN_NAMES] =
            core::array::from_fn(|span| TimestampWrites::of(q, span));
        // What outlives the frame, the impostors' bake and the air's tables,
        // is drawn and submitted ahead of it. From here on the frame only
        // describes itself.
        let ahead = g.encoder();
        let baked = self.impostor.bake(g, ahead, &self.decor, b);
        read.mark(baked);
        let mut ctx = RenderContext::new(g, ahead, &mut stamps);
        let (tables, exposed, copied) = self.atmosphere.draw(&mut ctx);
        read.mark(tables);
        if baked || tables || exposed || copied {
            g.submit(ahead);
        } else {
            release(ahead);
        }
        self.atmosphere.read_back(g);
        if clouds_changed {
            self.cloud.write(g);
        }
        // The world stands when all the frame draws from read as last time,
        // except its lists and its jitter, and the rays were cast again at
        // most at their own pixels.
        let still = read.end() && self.screen.kind() != FrameKind::cast && !self.screen.generates();
        let decided = FramePlan::of(FrameRead {
            partial,
            still,
            lists_same,
            clouds_damage,
            unmasked: self.screen.unmasked(),
            casts_px: self.screen.casts_px(),
            light_casts: self.screen.light_casts(),
        });
        let (quiet, masked) = (decided.quiet, decided.masked);
        if !masked {
            self.screen.whole();
        }
        if decided.unchanged {
            self.frame_read = read;
            let mut stat = self.stat(&draws, input.counted, None);
            // Nothing was drawn, so the frame has no number for a read to
            // name.
            stat.frame = 0.0;
            stat.ray_cnt = self.screen.take_cnts();
            self.touched = touched;
            return (FrameKind::unchanged, stat);
        }

        // In a gated frame, each listed draw that changed marks the screen's
        // tiles it covers where it was and where it is, and a caster also
        // marks the map's (pass/mark.rs). The CPU counts such a mark as
        // one it handed. The copies of what the marks read, as the last frame
        // drew it, are made before the frame's commands.
        let (screen_marking, casters_marking) = if masked {
            let k = &self.last_frame_uploads;
            let m = DamageInput {
                draws,
                changes: update.changes,
                touched: &touched,
                drift: [drift_at(&self.frame), drift_at(&self.last_frame_words)],
            };
            let characters = self.skinned_mesh.mark.prepare(g, &m, b, k);
            let decor = self.decor.mark.prepare(g, &m, b, k);
            let ghosts = self.decor.ghost_mark.prepare(g, &m, b, k);
            let impostors = self.impostor.mark.prepare(g, &m, b, k);
            let zones = self.location.zone_mark.prepare(g, &m, b, k);
            let beacons = self.location.beacon_mark.prepare(g, &m, b, k);
            (
                characters || decor || ghosts || impostors || zones || beacons,
                characters || decor,
            )
        } else {
            (false, false)
        };
        let mut read_uploads = core::mem::take(&mut self.read_uploads);
        self.marks_read(&draws, &mut read_uploads);
        self.last_frame_uploads.prepare(g, &read_uploads, b);
        // The light's map is drawn whole where it moved. Where the frame is
        // gated and the light's cast left the world as it was, the map is drawn
        // through its gate in the tiles the changed casters set, or not at
        // all. Only the GPU knows which tiles they set, so the map draws its
        // claimed tiles within the box they cover, which the GPU keeps and
        // counts (`TileGrid::rect_args`).
        // The CPU hands the map no mark. Through the gate the casters mark
        // their tiles by drawing, and a map drawn whole claims every tile by
        // its uniform. Its stamp is read by no frame while the map's
        // `damage_frames` is 1 (`TileGrid::damage`).
        let cast = self.screen.kind() != FrameKind::plain;
        let map_masked = masked && cast && !self.screen.light_casts();
        self.light_tiles.size(g, [SHADOW_SIZE; 2]);
        self.light_tiles.damage(g, (map_masked, false, false), None);
        let draws_map = self.shadows && (!map_masked || casters_marking);
        // Where the map's tiles changed through its gate, the GPU carries the
        // change to the screen's tiles whose pixels read them
        // (pass/propagate.rs). That happens only where a caster marks, and a
        // caster marks the screen's tiles too (`screen_marking`), so the CPU
        // counts the frame as marked.
        let propagating = draws_map && map_masked;
        if propagating {
            let spread = propagate::spread(SHADOW_MARGIN as f32);
            let light = &self.shadow.light_view_proj;
            self.propagate
                .prepare(g, &input.view_proj, light, spread, SHADOW_SIZE);
        }
        // Where only the listed draws that mark moved, the claimed tiles are
        // those that what moved marked, however much of the screen they and
        // the tiles still gathering cover. Where anything else moved, every
        // tile is claimed. The gate, the resolve and the finish draw these
        // tiles.
        self.resolve
            .tiles
            .damage(g, (masked, quiet, animated), sheets);
        if screen_marking {
            let now = self.resolve.tiles.now();
            self.resolve.tiles.marked_in(now, animated);
        }

        self.set_groups(g, world);
        // The frame proper changes nothing that outlives it, and it lets go
        // within itself of each object it makes. A frame too large for the
        // stream is then dropped whole, and the screen keeps the last frame.
        frame_begin();
        let encoder = g.encoder();
        let mut ctx = RenderContext::new(g, encoder, &mut stamps);
        // The screen's tiles' words come first, then its marks, then the
        // light's marks, each before anything reads them. A dropped
        // frame leaves them as they were, and the next frame is drawn whole.
        // A span times from its first pass to its last (gpu/api.rs
        // `TimestampWrites`), so the passes of the screen's damage (`damage`),
        // the shadow map's (`shadow_damage`) and the propagation (`propagate`) are
        // each encoded together.
        self.resolve.tiles.dispatch_damage(&mut ctx);
        if screen_marking {
            // A draw as it is is marked by this frame's words, and as it was by
            // the last frame's, so each mark has the jitter its draw is or was
            // drawn at.
            g.write_buf(self.last_frame_buf, 0, bytes(&self.last_frame_words));
            let frames = [self.marks[0].id(), self.marks[1].id()];
            // The world's depth is attached read only, with its stencil kept.
            // The marks test nothing and write no color.
            let desc = pass_desc(
                0,
                Load::Clear,
                world.depth_view,
                Load::ReadOnly,
                0.0,
                ctx.stamp(GpuSpanName::damage).pass(),
            )
            .load_stencil();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            self.skinned_mesh.draw_marks(&mut pass, frames, b);
            self.decor.draw_marks(&mut pass, frames, b, false);
            self.impostor.draw_marks(&mut pass, frames);
            self.location.draw_marks(&mut pass, frames);
            pass.end();
        }
        // In a frame through the gate, generated decor is drawn only where it
        // meets the tiles the frame claims. It is culled to them now that the
        // marks are made (`Generator::cull`).
        let generated = if masked {
            let tiles = &self.resolve.tiles;
            self.screen
                .cull_decor(&mut ctx, Mask::Screen, self.frame_buf, tiles);
            self.screen
                .generated()
                .map(|d| d.through_mask(Mask::Screen))
        } else {
            self.screen.generated()
        };
        // The same from the light, where the map is drawn through its gate.
        // The casters draw themselves to set the words of the map's tiles
        // their shadows change, and the propagation reads those words.
        if propagating {
            let (_, map) = self.shadow.frame_and_map();
            let desc = pass_desc(
                0,
                Load::Clear,
                map,
                Load::ReadOnly,
                1.0,
                ctx.stamp(GpuSpanName::shadow_damage).pass(),
            )
            .load_stencil();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            // The light's frame has no jitter, and it stands still in a frame
            // whose map is drawn through its gate. One group therefore serves
            // a draw as it is and as it was.
            let frames = [self.light_mark.id(); 2];
            self.skinned_mesh.draw_marks(&mut pass, frames, b);
            self.decor.draw_marks(&mut pass, frames, b, true);
            pass.end();
            // The GPU counts the box the marks cover into the gate's draw.
            self.light_tiles.dispatch_rect_args(&mut ctx);
            // Generated decor is drawn into the map only where it meets the
            // tiles the map claims, as the screen's gate draws it
            // (`Generator::cull`).
            let (light_frame, _) = self.shadow.frame_and_map();
            self.screen
                .cull_decor(&mut ctx, Mask::Light, light_frame, &self.light_tiles);
        }
        // The generated decor as the map draws it: through the map's gate
        // where the map is drawn through one.
        let map_generated = if propagating {
            self.screen.generated().map(|d| d.through_mask(Mask::Light))
        } else {
            self.screen.generated()
        };
        if propagating {
            self.propagate
                .dispatch(&mut ctx, self.resolve.tiles.squares());
        }
        self.screen.dispatch(&mut ctx);
        // The shadow map holds the world's depth seen from the light, in the
        // tiles a receiver reads. The rays hold the world it was cast with,
        // and the decor and the characters are drawn over it. Where
        // `map_masked`, it is drawn through its gate, with its stencil
        // numbered as the world pass's is.
        if let Some(reads) = self.screen.light_read().filter(|_| draws_map) {
            let tiles = &mut self.light_tiles;
            let mut pass = self.shadow.begin(&mut ctx, cast, tiles, map_masked, reads);
            let light = self.shadow.light_group;
            self.decor
                .draw_generated(&mut pass, map_generated, b, Stage::Depth, light);
            self.decor.draw(&mut pass, &draws, b, Stage::Depth, light);
            self.skinned_mesh.draw(
                &mut pass,
                draws.of(PIPELINE_SKINNED),
                b,
                Stage::Depth,
                light,
            );
            pass.end();
        }
        let canvas_view = g.cur(world.handle);
        // The occlusion is a field over the screen, as the light's map is over
        // the world (pass/ao.rs). It is found again from the depth the static
        // opaque draws lay where what it reads moved, and held where nothing
        // did. A gated frame moves nothing it reads, since what the core
        // animates is drawn after it.
        let ao_found = self.screen_ao && {
            let a = &mut self.ao_read;
            a.begin();
            // The jitter is not read, as the light's receivers do not read it
            // either. An occlusion found at one jitter stands under the next,
            // since the field is coarser than the pixel the jitter moves.
            a.read(&self.frame[..JITTER]);
            a.read(self.ao.words());
            a.read(&[
                world.width,
                world.height,
                u32::from(self.screen.generated().is_some()),
            ]);
            // The world's depth is the rays', so the occlusion is laid again by
            // a cast where the world the rays read moved, or by decor generated
            // again. A cast where only the jitter moved (`FrameKind::px`) does
            // not lay it again.
            a.mark(ao_remade || self.screen.generates() || self.screen.kind() == FrameKind::cast);
            !a.end() && !masked
        };
        // A frame not drawn whole keeps the rest as the last frame left it.
        // Its gate first clears the claimed tiles and writes this frame's
        // number into their stencil, and every draw after it passes only
        // there (`Stencil`). The stencil is kept from frame to frame and
        // cleared only when the numbers come round. A frame drawn whole
        // clears it, and its number is 0.
        let load = if masked { Load::Load } else { Load::Clear };
        let desc = pass_desc(
            world.color_view,
            load,
            world.depth_view,
            load,
            0.0,
            ctx.stamp(GpuSpanName::world).pass(),
        );
        let (desc, number) = self.resolve.tiles.mask(desc, masked);
        let pass = g.pass(encoder, &desc);
        g.declare_pass(pass, sequence::LIT_PASS);
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(number) = number {
            pass.set_stencil_reference(number);
            let [across, down] = self.resolve.tiles.squares();
            pass.set_pipeline(self.mask.id);
            pass.set_bind_group(0, self.mask_group.id(), &[]);
            pass.draw(6, across * down, 0, 0);
        }
        // First the world that stands while the eye does, which is what the
        // occlusion reads. The impostors have no depth pipeline, so they are
        // drawn only here and cast no shadow.
        self.decor
            .draw_generated(&mut pass, generated, b, Stage::Lit, self.model_group.frame);
        self.screen.draw(&mut pass);
        // The impostors go last among the opaque pipelines, so the pixels
        // behind the alpha-tested fragments they discard are already filled.
        self.impostor
            .draw_generated(&mut pass, generated, GROUPS, self.model_group.frame);
        if ao_found {
            // The pass is split around the occlusion only where the occlusion
            // is found again, in a frame drawn whole. The occlusion reads the
            // depth just laid, and the rest is drawn over it in a new pass
            // that loads the color and the depth. The stencil stays 0, as the
            // frame drawn whole cleared it.
            pass.end();
            self.ao.run(&mut ctx);
            let desc = pass_desc(
                world.color_view,
                Load::Load,
                world.depth_view,
                Load::Load,
                0.0,
                ctx.stamp(GpuSpanName::world_rest).pass(),
            );
            let rest = g.pass(ctx.encoder, &desc);
            g.declare_pass(rest, sequence::LIT_PASS);
            pass = TrackedRenderPass::new(g, rest);
        }
        // The occlusion's map is laid over the color in the tiles drawn again,
        // one quad a tile, before what the core animates is drawn.
        if self.screen_ao {
            let [across, down] = self.resolve.tiles.squares();
            self.ao.apply(&mut pass, across * down);
        }
        // The sky is drawn at the far plane after the opaque world, so only the pixels the
        // world left empty run its shader.
        self.sky.draw(&mut pass);
        self.star.draw(&mut pass);
        // The characters, whose clips play, and the clouds the core lists, the
        // cumulus, which the wind carries, are drawn over the world against
        // its depth. The occlusion is not laid over them.
        let frame_group = self.model_group.frame;
        self.skinned_mesh.draw(
            &mut pass,
            draws.of(PIPELINE_SKINNED),
            b,
            Stage::Lit,
            frame_group,
        );
        self.decor
            .draw(&mut pass, &draws, b, Stage::Lit, frame_group);
        self.impostor
            .draw(&mut pass, draws.of(PIPELINE_IMPOSTOR), b, frame_group);
        self.cloud.draw(&mut pass);
        self.location.draw_beacon(&mut pass, &draws, frame_group);
        self.location.draw_zone(&mut pass, &draws, b);
        self.draw_ghost_box(&mut pass, &draws);
        self.decor
            .draw_ghosts(&mut pass, &draws, b, self.model_group.frame);
        pass.end();
        let squares = self.resolve.tiles.squares();
        // The frame words the listed draws were drawn with are kept, so the
        // next frame can mark the draws where they were. The draws as they
        // were come with the next turn's changes (gpu/draw.rs `Change`).
        self.last_frame_words = self.frame;
        self.last_frame_uploads
            .keep(g, ctx.encoder, &read_uploads, &touched, b);
        // The depth's bounds that the propagation reads are set in the tiles
        // the world's pass drew, which are the claimed ones. There are none
        // without the shadows, and the propagation runs only with them. The
        // frame the shadows come on in moves the frame words
        // (`SHADOW_STRENGTH`), so that frame is drawn whole and sets every
        // square before any propagation reads them.
        if self.shadows {
            self.propagate.set_bounds(&mut ctx, squares);
        }
        let gathered = self.resolve.draw(&mut ctx, world, masked);
        // The finish runs at the canvas's size, in its own span. A frame not
        // drawn whole finishes again only its claimed tiles
        // (screen_tile.wgsl), unless the debug pass drew into the screen's
        // depth last frame.
        self.finish.draw(
            &mut ctx,
            GpuSpanName::finish,
            world,
            gathered,
            canvas_view,
            Some((&self.resolve.tiles, masked && !self.debug_drew)),
        );
        self.marker
            .draw(&mut ctx, canvas_view, world.screen_depth_view, &draws, b);
        self.debug_drew = self.debug.draw(
            &mut ctx,
            canvas_view,
            world.screen_depth_view,
            &draws,
            b,
            input.camera_gizmo,
        );
        self.draw_character_surfaces(
            &mut ctx,
            Self::character_surfaces(input.character_surfaces),
            uploads_changed || tables || exposed,
        );
        let written = stamps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_written())
            .fold(0, |m, (k, _)| m | 1 << k);
        let read_slot = g.submit_frame(encoder, written);
        release(canvas_view);
        // The read that landed was asked by an earlier frame and times that
        // frame. This frame's figures are kept by the read it was asked in,
        // until that read lands.
        let gpu = g.stamps_complete().filter(|_| q != 0).map(|r| GpuRead {
            frame: self.read_frame[r],
            spans: core::array::from_fn(|k| g.gpu_ms(k as u32)),
            busy_ms: g.gpu_busy_ms(),
            px: self.read_px[r],
        });
        let frame = DRAWN_FRAMES.get(|| 0.0);
        *frame += 1.0;
        if let Some(r) = read_slot {
            self.read_px[r] = f64::from(world.width) * f64::from(world.height);
            self.read_frame[r] = *frame;
        }
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
            self.screen.on_dropped();
            for pass in self.character_passes.iter_mut().flatten() {
                pass.read.forget();
            }
            read.forget();
            self.ao_read.forget();
            // The stencil holds what the last drawn frame left, so the next
            // gated frame clears it.
            self.resolve.tiles.on_dropped();
            self.light_tiles.on_dropped();
            self.last_frame_uploads.forget();
        } else {
            self.resolve.on_drawn(masked);
            // Once anything moved, the frames ask for turns until the jitter's
            // sequence has run through, since nothing else asks for those
            // frames while the world stands. What moves on its own (a
            // character, the clouds, the location indicator) wakes the turn at
            // its own steps and is drawn with the jitter held
            // (`Resolve::asks`), so it asks for no frame between.
            if self.resolve.asks() && (quiet || !animated) {
                REDRAW.set(true);
            }
            if self.screen.generates() {
                self.screen.on_drawn();
            }
        }
        read.end();
        self.frame_read = read;
        let kind = if skipped {
            FrameKind::dropped
        } else if masked {
            FrameKind::masked
        } else {
            self.screen.kind()
        };
        let mut stat = self.stat(&draws, input.counted, gpu);
        stat.ray_cnt = self.screen.take_cnts();
        self.touched = touched;
        self.read_uploads = read_uploads;
        (kind, stat)
    }

    /// The character surfaces drawn this frame: those whose handle still names
    /// a character surface.
    fn character_surfaces(
        surfaces: &[(u32, Draw, Image, u32)],
    ) -> impl Iterator<Item = &(u32, Draw, Image, u32)> {
        surfaces
            .iter()
            .filter(|p| surface(p.0).is_some_and(|s| s.role == surface_role::CHARACTER))
    }

    /// Makes each character surface's targets, pass and skin groups, so that
    /// drawing makes none: an object made inside a frame that is dropped would
    /// be gone.
    fn prepare_character_surfaces(&mut self, g: &Gpu, surfaces: &[(u32, Draw, Image, u32)]) {
        for &(id, (rec, _), image, _) in Self::character_surfaces(surfaces) {
            let Some(s) = surface(id) else { continue };
            targets(g, s);
            let slot = handle_slot(id) as usize;
            if self.character_passes[slot].is_none() {
                self.character_passes[slot] = Some(CharacterPass::new(
                    g,
                    &mut self.pipelines,
                    &self.shadow,
                    self.atmosphere.entries(),
                    self.align,
                ));
            }
            let pass = self.character_passes[slot]
                .as_mut()
                .or_bug(Errno::ENOTRECOVERABLE);
            pass.skinned_mesh.set_skins(&[(rec[2], image)]);
            pass.skinned_mesh
                .prepare(g, core::iter::once(&rec), uploads());
            pass.set_groups(g, s);
        }
    }

    /// Draws each character surface whose picture's makings moved
    /// (`CharacterPass::draw`). `changed` says that something without words
    /// moved: the models' uploads, the air's tables or its exposure.
    fn draw_character_surfaces<'p>(
        &mut self,
        ctx: &mut RenderContext,
        surfaces: impl Iterator<Item = &'p (u32, Draw, Image, u32)>,
        changed: bool,
    ) {
        for (id, draw, image, bg) in surfaces {
            let Some(s) = surface(*id) else { continue };
            let Some(pass) = self.character_passes[handle_slot(*id) as usize].as_mut() else {
                continue;
            };
            pass.draw(ctx, s, self.frame, draw, *image, *bg, changed);
        }
    }

    /// The frame's figures. The draws counted are what the scheduler asked
    /// for, so a draw passed over because its upload, skin or sprite is not on
    /// the device yet still counts. They are counted only while `counted`,
    /// which only the profiler sets, so a frame no one reads counts none.
    fn stat(&self, draws: &DrawTable, counted: bool, gpu: Option<GpuRead>) -> FrameStat {
        let b = uploads();
        let mut stat: DrawStat = [[0.0; 2]; DRAW_PIPELINES.len()];
        if counted {
            draws.count(PIPELINE_GHOST_BOX, &mut stat, |d| d[1] * BOX_VERTICES / 3);
            self.decor.count(draws, b, &mut stat);
            self.impostor.count(draws, &mut stat);
            self.skinned_mesh.count(draws, b, &mut stat);
            self.location.count(draws, &mut stat);
            self.marker.count(draws, &mut stat);
            self.debug.count(draws, &mut stat);
        }
        FrameStat {
            gpu,
            frame: *DRAWN_FRAMES.get(|| 0.0),
            uploads: b.cnt as f64,
            upload_bytes: b.bytes as f64,
            pack_bytes: self.pack.bytes as f64,
            draw_stat: stat,
            light_changed: f64::from(self.screen.light_changed()),
            decor_rects: f64::from(self.screen.decor_rects()),
            ray_cnt: None,
        }
    }
}

/// The entries of a mark's group 0 (tile_mark.wgsl): the frame words
/// `frame`, and the words, the uniform and the marks' box of `tiles`.
fn mark_entries(frame: u32, tiles: &TileGrid) -> [Entry; 4] {
    let (words, uniform) = tiles.this_frame();
    [
        buf_entry(0, frame),
        buf_entry(30, words),
        buf_entry(31, uniform),
        buf_entry(32, tiles.new_damage_rect()),
    ]
}

/// Makes `s`'s targets at its size. A new size is applied once it has held
/// for `SETTLE_TURNS` turns; until then the page scales the old buffer.
fn targets(g: &Gpu, s: &mut Surface) {
    let ((width, height), screen) = (s.render_px(), s.screen_px());
    if !s.configured {
        g.configure(s.handle);
        s.configured = true;
        s.depth = 0;
    }
    let made = s.depth != 0;
    if made && s.width == width && s.height == height && s.screen == screen {
        s.waited_turns = 0;
        return;
    }
    if made {
        if s.wanted_sizes != ((width, height), screen) {
            s.wanted_sizes = ((width, height), screen);
            s.waited_turns = 0;
        }
        s.waited_turns += 1;
        if s.waited_turns < SETTLE_TURNS {
            REDRAW.set(true);
            return;
        }
    }
    s.waited_turns = 0;
    g.resize(s.handle, screen.0, screen.1);
    for id in s.targets() {
        release(id);
    }
    let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
    s.depth = g.texture(width, height, 1, api::MASKED_DEPTH, 1, usage);
    s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
    s.depth_only_view = g.depth_only_view(s.depth);
    s.color = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
    s.color_view = g.view(s.color, false, 0, 1, 0, 1);
    s.screen_depth = g.texture(screen.0, screen.1, 1, Format::depth32float, 1, usage);
    s.screen_depth_view = g.view(s.screen_depth, false, 0, 1, 0, 1);
    // Only the world surface holds the cast's rays, the color gathered over
    // frames and the kept picture.
    (s.rays, s.rays_view) = (0, 0);
    (s.history, s.history_view) = ([0; 2], [0; 2]);
    (s.picture, s.picture_view) = (0, 0);
    if s.role == surface_role::WORLD {
        s.picture = g.texture(screen.0, screen.1, 1, g.canvas(), 1, usage);
        s.picture_view = g.view(s.picture, false, 0, 1, 0, 1);
        for k in 0..2 {
            s.history[k] = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
            s.history_view[k] = g.view(s.history[k], false, 0, 1, 0, 1);
        }
        // The rays are written by the cast and read by the screen pass.
        let rays = texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING;
        s.rays = g.texture(width, height, ray::WORDS, Format::r32uint, 1, rays);
        s.rays_view = g.view(s.rays, true, 0, 1, 0, ray::WORDS);
    }
    s.width = width;
    s.height = height;
    s.screen = screen;
}

/// Holds the edits named by the decor of a cast that was handed and never
/// drawn, for the next frame that is drawn.
fn carry_edits(cast: &Option<Cast>) {
    let Some(d) = cast.as_ref().map(|c| &c.decor) else {
        return;
    };
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        r.screen.carry_edits(d.edits);
    }
}

/// Draws a frame from `input`. Without a device or the world's canvas, the
/// lists are recorded for the frame that has both.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == surface_role::WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), device(), world) else {
        carry_edits(&input.cast);
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), &input.update);
        if let Some(r) = RENDERER.get(|| None).as_mut() {
            r.put_off();
        }
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: false,
            stat: None,
        };
    };
    // The renderer is made in three steps before it draws (gpu/pipeline.rs).
    // First its shaders' modules are made, a few a turn. Then the renderer
    // is made, and it asks for every pipeline by WebGPU's async call. Then
    // nothing happens until each pipeline is made. A turn before that draws
    // nothing and asks for the next.
    let slot = RENDERER.get(|| None);
    if slot.is_none() {
        let making = MAKING.get(|| None);
        if making
            .get_or_insert_with(PipelineCache::new)
            .make_modules(g)
        {
            let pipelines = making.take().or_bug(Errno::ENOTRECOVERABLE);
            *slot = Some(Renderer::new(g, device, pipelines));
        }
    }
    if !slot.as_mut().is_some_and(|r| r.ready(g)) {
        carry_edits(&input.cast);
        take_lists(slot.as_ref().map(|_| g), &input.update);
        if let Some(r) = slot.as_mut() {
            r.put_off();
        }
        // The modules are made a few a turn, so another turn is wanted while
        // there is no renderer. A pipeline asked for wakes the turn as it is
        // made (`complete`).
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: slot.is_none(),
            stat: None,
        };
    }
    let r = slot.as_mut().or_bug(Errno::ENOTRECOVERABLE);
    uploads().restore(g);
    let (kind, stat) = r.frame(g, world, input);
    FrameOutput {
        kind,
        redraw: *REDRAW.get(|| false),
        stat: Some(stat),
    }
}

/// Whether what the device finished since this last ran wants a frame. That
/// is an image that landed and is to be taken (`api::images_complete`), a
/// pipeline made, which the progress shows and the first frame waits on, or
/// the air's measures (`Atmosphere::complete`). Each wakes the turn when its
/// fence is stored (xpute `RESULT`), so none is polled.
pub fn complete() -> bool {
    let images = api::images_complete();
    let made = RENDERER.get(|| None).as_mut().is_some_and(|r| {
        let was = r.pipelines.share();
        let pipelines =
            !r.pipelines.is_made() && (r.pipelines.poll() || r.pipelines.share() != was);
        pipelines || r.atmosphere.complete()
    });
    images || made
}

/// Applies the lists' frees, uploads and rewrites to the upload table, and
/// says whether an upload was let go, made or rewritten. Uploads listed with
/// no device wait in the table for the next one.
fn take_lists(g: Option<&Gpu>, update: &DrawUpdate) -> bool {
    let b = uploads();
    // Frees first, so the table never holds an upload beside its replacement.
    for &handle in update.frees {
        b.remove(handle);
    }
    for u in update.uploads.as_chunks::<UPLOAD_WORDS>().0 {
        b.insert(g, u[0], u[1], u[2], u[3], u[4]);
    }
    for r in update.rewrites.as_chunks::<REWRITE_WORDS>().0 {
        b.rewrite(g, r[0], r[1], r[2], r[3]);
    }
    !(update.frees.is_empty() && update.uploads.is_empty() && update.rewrites.is_empty())
}

#[cfg(test)]
#[path = "renderer.test.rs"]
mod test;
