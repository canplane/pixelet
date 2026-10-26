// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec_core::eye;
use pxl_spec_core::ray;
use pxl_spec_core::scatter::GROUPS;
use pxl_spec_core::ways;
use pxl_spec_core::window::{self, FreshSquare};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::api_spec::surface_role;
use crate::frame::draw::{
    census_slot, DrawCensus, Lists, DRAW_PIPELINES, DRAW_WORDS, PIPELINE_CAMERA_GRID,
    PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_MARKER, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS,
    UPLOAD_WORDS,
};
use crate::frame::shader::record::PIPELINE_SKY;
use crate::frame::shader::word::frame::{
    CAMERA_POS, CELL, CORNER_AO, EDGE_STRENGTH, EDGE_WIDTH, GIZMO, LIGHT_VIEW_PROJ, LOW_SPEC, MAP,
    RIM_INTENSITY, SCREEN_AO, SHADOW_RIM, SHADOW_STRENGTH, SHADOW_TEXEL, VIEWPORT, VIEW_PROJ,
    VIGNETTE,
};
use crate::frame::shader::word::{sky, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, device, device_lost, frame_begin, frame_commit, gpu,
    pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes, Device, Entry,
    Format, Gpu, Image, Load, Stamp,
};
use crate::gpu::atmosphere::{Atmosphere, Upload};
use crate::gpu::cloud::CloudPass;
use crate::gpu::grown::DecorFrame;
use crate::gpu::image_map::ImageMap;
use crate::gpu::mip::Mips;
use crate::gpu::pack::Pack;
use crate::gpu::pass::case::Case;
use crate::gpu::pass::debug::DebugPass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::marker::MarkerPass;
use crate::gpu::pass::model::{frame_layout, skins_lost, FrameGroups, Stage};
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::ray::{RaysPass, ORACLE_COUNTS};
use crate::gpu::read::Read;
use crate::gpu::shadow::Shadow;
use crate::gpu::sheet::shelf::Sheet;
use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::star::StarPass;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::gpu::upload::{uploads, Draws, Uploads};
use crate::look::environment::{ENV_AIR, ENV_STARS, ENV_SUN};

const BOX_VERTICES: u32 = 36;
const DRAWS_INITIAL: u32 = 256 * DRAW_FLOATS as u32;

const _: () = assert!(
    GPU_SPAN_NAMES <= 32,
    "a span a bit of the mask the host is told"
);
const SPAN_AIR: usize = GpuSpanName::air as usize;
const SPAN_PREPASS: usize = GpuSpanName::prepass as usize;
const SPAN_AO: usize = GpuSpanName::ao as usize;
const SPAN_WORLD: usize = GpuSpanName::world as usize;
const SPAN_MARKER: usize = GpuSpanName::marker as usize;
const SPAN_DEBUG: usize = GpuSpanName::debug as usize;
const SPAN_CASE: usize = GpuSpanName::case as usize;
const SPAN_FILL: usize = GpuSpanName::fill as usize;
const SPAN_SHADOW: usize = GpuSpanName::shadow as usize;

type Frame = [f32; FRAME_FLOATS];

pub struct FrameInput<'a> {
    pub lists: Lists<'a>,
    pub environment: &'a [f32],
    /// 0 while it loads.
    pub decor: &'a [(u32, Option<Image>, Extent)],
    pub skins: &'a [(u32, Image)],
    /// Each display case: its surface, its actor's palette, which names its
    /// draw, and the skin.
    pub cases: &'a [(u32, u32, Image)],
    /// The texture pack's image; none while it loads.
    pub pack: fn() -> Option<Image>,
    pub moon: fn() -> Option<Image>,
    pub earth: fn() -> Option<Image>,
    pub earth_tiles: EarthTiles<'a>,
    /// The terrain's held tiles, whose cells the GPU draws whether or not
    /// an eye casts.
    pub ground: GroundSquares<'a>,
    /// The eye's rays cast on the GPU into the world surface's rays the
    /// screen pass draws, a drawn pixel each; none without an eye, and none
    /// drawn.
    pub cast: Option<Cast<'a>>,
    pub sheet: &'a mut Sheet,
    pub text: TextOf,
    /// Whether the surfaces take a frame now without blocking; one admitted
    /// is drawn. Asked only once nothing else stops the frame.
    pub admit: fn() -> bool,
    /// Whether its draws are counted (`FrameStat::census`): the profiler's
    /// alone, so a frame it does not record counts none.
    pub counted: bool,
}

/// The earth layer's tiles as the GPU reads them: the floor's window, its
/// south-west tile first, and per tile its layer plus one; `upload` hands each
/// tile that has come to the writer while the writer takes it.
pub struct EarthTiles<'a> {
    pub window: [i32; 2],
    pub layers: &'a [u32],
    pub upload: &'a mut Upload,
}

/// The ground's windows as the GPU holds them (pxl-spec-core `window`,
/// their squares in `SQUARES` berths): the squares to hand it, and where
/// their cells come back.
pub struct GroundSquares<'a> {
    /// Hands each square whose cells the GPU is to draw to the writer.
    pub upload: &'a mut TerrainUpload,
    /// Told each square whose cells came back, and the fill they are of
    /// (`CellsBack::ticket`).
    pub landed: &'a mut CellsLanded,
    /// Where the eye's windows folded over their squares come back to in
    /// the module's memory (`joined_cs`; pxl-spec-core
    /// `window::EYE_JOINED_WORDS`).
    pub joined: u32,
    /// The ways' pool's families at first and at most (pxl-kernel domain/layer/terrain/param.rs
    /// `WAYS_POOL_BYTES`): it starts at the first and doubles each time a
    /// build runs out, never past the most, nor `ways::tree::POOL_FAMILIES`
    /// (`GroundBuffers::grow`).
    pub pool: [u32; 2],
    /// The families each handed square's scratch holds a tree of, at first and
    /// at most (pxl-kernel domain/scene/param.rs `WAYS_SCRATCH_BYTES`): it starts at the
    /// first and doubles each time a build outgrows it, never past the most,
    /// nor `ways::tree::SQUARE_FAMILIES` (`GroundBuffers::grow_scratch`).
    pub scratch: [u32; 2],
    /// A read's records' room at first, in bytes: every read's is doubled
    /// past a square's records that outgrew it, up to
    /// `ways::tree::RECORDS_BYTES` (`GroundBuffers::record_bytes`).
    pub records: u32,
}

/// How a square's fill landed: its cells are back in every case.
pub enum Landing<'a> {
    /// Its ways' tree built: the families it holds, their count first, a
    /// record each (pxl-spec-core `ways::tree::lay`).
    Built(&'a [u32]),
    /// To be filled again, for want of room the renderer grows: its tree not
    /// built, the pool or the scratch it was built in having run out
    /// (`Short`), or built but its records past its read's room, the tree
    /// the refill frees.
    Again,
    /// Its tree refused, the pool or the scratch at its most having run out,
    /// or the square past its own (`ways::tree::past`): the square stays
    /// absent.
    Refused,
}

/// The pool's records the renderer keeps on the heap at their worst, in
/// the buddy's blocks: every read's room at a square's most
/// (`GroundBuffers::room`), each a power of two as the heap gives it.
pub const RECORD_ROOMS_BYTES: u64 =
    window::READS as u64 * (ways::tree::RECORDS_BYTES as u64).next_power_of_two();

/// A square whose cells came back, the fill they are of
/// (`CellsBack::ticket`), and how its ways' tree landed.
pub type CellsLanded = dyn FnMut(u32, u32, Landing<'_>);

/// The GPU's cast this frame (frame/shader/cast.gen.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-spec-core `eye`; pxl-kernel domain/view/lens.rs `gpu_cast`,
    /// domain/scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// The windows' table (pxl-spec-core `window::Slot`; pxl-kernel
    /// domain/layer/terrain/window.rs `table`), four words a row: every window's
    /// slots, the eye's first, from the top's, each the square it wants, X
    /// and Y, the berth holding it plus one while present (`Slot::berth`)
    /// and the fill it is on its way from; then each window's first square
    /// (`window::ORIGIN_AT`).
    pub window: &'a [i32],
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row (pxl-kernel domain/scene/cast.rs `shards`); read where it lies,
    /// unwritten until the page replays the turn.
    pub shards: &'a [[i32; 4]],
    pub overwrites: Overwrites<'a>,
    /// The decor the GPU grows and draws over the same world (gpu/grown.rs).
    pub decor: DecorFrame<'a>,
}

/// The overwrites' tree (pxl-kernel domain/layer/object/store.rs) as it lives: its
/// heap and data lanes, unwritten until the page replays the turn, the room
/// either has before it grows, the revision they are at, and the families
/// written since `since`, the revision they were last handed at, by their
/// first slots (`GpuLanes`).
pub struct Overwrites<'a> {
    pub heap: &'a [u32],
    pub data: &'a [u32],
    pub room: usize,
    pub revision: u32,
    pub since: u32,
    pub written: &'a [u32],
}

/// Takes a square while the turn's stream has room for it.
pub type TerrainWrite<'a> = dyn FnMut(FreshSquare<'_>) -> bool + 'a;

pub type TerrainUpload = dyn for<'w> FnMut(&mut TerrainWrite<'w>);

pub struct FrameOutput {
    /// What the frame turn made: decided where each case is (no device or
    /// surface, not admitted, dropped from the stream, and the cast's kind).
    pub kind: FrameKind,
    /// A surface waits on a size to settle, so another turn is wanted.
    pub redraw: bool,
    pub stat: Option<FrameStat>,
}

#[derive(Clone, Copy)]
pub struct FrameStat {
    /// Whether a GPU read landed with this frame: its spans are then that
    /// read's and `gpu_px` the world's pixels the frame it timed drew, what
    /// the spans were measured over; else both are 0, as nothing was
    /// measured with this frame.
    pub gpu_read: bool,
    pub gpu_px: f64,
    /// The upload table's uploads and their bytes, the depth targets' bytes
    /// and the texture pack's: each as counted, in bytes.
    pub uploads: f64,
    pub upload_bytes: f64,
    pub depth_bytes: f64,
    pub pack_bytes: f64,
    /// Every draw's, counted only while `counted`: its sums are the frame's
    /// draws and triangles.
    pub census: DrawCensus,
    pub gpu_spans: [f64; GPU_SPAN_NAMES],
    /// The oracle's sums since boot (gpu/ray/cast.rs `Oracle`).
    pub oracle: [f64; ORACLE_COUNTS + 1],
}

/// A frame's GPU time: its spans' sum, the passes timed. The span from the
/// first to the last would count what lies between submits.
pub fn gpu_ms(spans: &[f64]) -> f64 {
    spans.iter().sum()
}

/// The passes drawn at the world's own pixels, whose cost goes with them:
/// its depth prepass, its occlusion, its drawing and its cast. The air's
/// tables, the fill and the decor's growth, the thumbnails, markers and debug
/// cost what they cost at any resolution.
const PER_PIXEL: [GpuSpanName; 4] = [
    GpuSpanName::prepass,
    GpuSpanName::ao,
    GpuSpanName::world,
    GpuSpanName::cast,
];

/// A frame's GPU time in the passes drawn at the world's pixels (`PER_PIXEL`):
/// what its drawn resolution can buy back.
pub fn pixels_ms(spans: &[f64]) -> f64 {
    PER_PIXEL.iter().map(|&s| spans[s as usize]).sum()
}

/// The ray under NDC (x, y) is `x·col0 + y·col1 + col2`, taken in f64: a
/// direction unprojected from two points in the shader holds only ~1e-3 rad,
/// which the secant carries out to kilometres at the horizon.
fn put_ray(out: &mut [f32], at: usize, view_proj: &Mat4, eye: &V3) {
    let mut inv = mat4::new();
    mat4::invert(&mut inv, view_proj);
    let point = |x: f64, y: f64| -> V3 {
        let clip = [x, y, 0.5, 1.0];
        let row = |r: usize| (0..4).map(|c| inv[c * 4 + r] * clip[c]).sum::<f64>();
        let w = row(3);
        [row(0) / w, row(1) / w, row(2) / w]
    };
    let c = point(0.0, 0.0);
    for (k, (to, from)) in [(point(1.0, 0.0), c), (point(0.0, 1.0), c), (c, *eye)]
        .into_iter()
        .enumerate()
    {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3]
            .iter_mut()
            .zip(v3::sub(&to, &from))
        {
            *o = e as f32;
        }
    }
}

static RENDERER: Global<Option<Renderer>> = Global::new();

/// Each light's step, the sun's first: the height it moves before the sky as
/// drawn moves a display level (gpu/atmosphere.rs `steps`); none before any
/// view's slope has landed.
pub fn sky_steps() -> [Option<f32>; 2] {
    RENDERER
        .get(|| None)
        .as_ref()
        .map_or([None; 2], |r| r.atmosphere.steps())
}

static REDRAW: Global<bool> = Global::new();

pub fn gpu_lost() {
    RENDERER.set(None);
    skins_lost();
    device_lost();
    uploads().lose();
    surfaces_lost();
}

pub fn surface_closed(s: Surface) {
    for id in s.targets() {
        release(id);
    }
    let slot = handle_slot(s.handle) as usize;
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        if let Some(case) = r.cases[slot].take() {
            case.release();
        }
    }
}

pub struct Renderer {
    align: u32,
    /// The spans' queries (`Gpu::spans`); 0 where the device keeps no
    /// timestamps, and no frame is timed.
    query_set: u32,
    /// The world's pixels of the frame whose spans' read is on its way: one
    /// read at most is (`Gpu::submit_frame`).
    read_px: f64,
    pipelines: Pipelines,
    frame_buffer: u32,
    frame: Frame,
    sky_buffer: u32,
    sky: [f32; SKY_FLOATS],
    sky_built: Built,
    sky_group: u32,
    ghost: Built,
    draws_buffer: u32,
    draws_bytes: u32,
    bind_group: u32,
    models: FrameGroups,
    shadow: Shadow,
    /// Whether the sun's map is cast and drawn this frame (`STAGE_AT::SHADOWS`).
    shadows: bool,
    ao: ScreenAo,
    screen_ao: bool,
    atmosphere: Atmosphere,
    stars: StarPass,
    clouds: CloudPass,
    mips: Mips,
    pack: Pack,
    moon: ImageMap,
    decor: DecorPass,
    impostor: ImpostorPass,
    skinned: SkinnedPass,
    puck: PuckPass,
    marker: MarkerPass,
    debug: DebugPass,
    screen: RaysPass,
    /// The frame's whole input as the last frame drew it (stage 2): the
    /// frame's, the sky's and the stars' words, the air's, the surface's
    /// textures, the draws and the uploads they read, the markers' sheet, the
    /// clouds' words and the cases, and what came or was drawn again without
    /// words.
    frame_read: Read,
    /// What the occlusion was last made from: the frame's words, its own,
    /// the surface and the decor its prepass draws. The frame is drawn
    /// again for more than these, and the occlusion only where they moved.
    ao_read: Read,
    cases: [Option<Case>; SURFACES],
    order: Vec<u32>,
}

impl Renderer {
    fn new(g: &Gpu, device: Device) -> Renderer {
        let mut pipelines = Pipelines::new();
        let frame_buffer = g.buffer(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let sky_buffer = g.buffer(
            word_bytes(SKY_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buffer);
        let atmosphere = Atmosphere::new(g, &mut pipelines, frame_buffer, sky_buffer);
        let air = atmosphere.entries();
        let stars = StarPass::new(g, &mut pipelines, frame_buffer, air);
        let clouds = CloudPass::new(g, &mut pipelines, frame_buffer, sky_buffer, air);
        let mips = Mips::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let pack = Pack::new(g);
        let sky_built = pipelines.build(g, PIPELINE_SKY, g.canvas());
        let moon = ImageMap::new(g, MOON_TEXELS, MOON_TEXELS);
        let mut sky_entries = vec![
            buffer_entry(0, frame_buffer),
            buffer_entry(19, sky_buffer),
            view_entry(2, moon.view),
            sampler_entry(3, moon.sampler),
        ];
        sky_entries.extend(air);
        let sky_group = g.bind_group(sky_built.layouts[0], &sky_entries);
        let draws_buffer = g.buffer(
            DRAWS_INITIAL * 4,
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let shadow = Shadow::new(g, &mut pipelines);
        let models = FrameGroups::new(
            g,
            frame_layout(g, &mut pipelines),
            frame_buffer,
            shadow.entries(1),
            [ao.blank_entry(7); 2],
            air,
        );
        let query_set = if device.timestamps { g.spans() } else { 0 };
        let mut r = Renderer {
            align: device.align,
            query_set,
            read_px: 0.0,
            ghost: pipelines.build(g, PIPELINE_GHOST_BOX, g.canvas()),
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned: SkinnedPass::new(g, &mut pipelines, device.align),
            puck: PuckPass::new(g, &mut pipelines, frame_buffer, air, device.align),
            marker: MarkerPass::new(g, &mut pipelines, frame_buffer, device.align),
            debug: DebugPass::new(g, &mut pipelines, frame_buffer, sky_buffer, device.align),
            screen: RaysPass::new(g, &mut pipelines),
            frame_read: Read::default(),
            ao_read: Read::default(),
            pipelines,
            frame_buffer,
            frame: [0.0; FRAME_FLOATS],
            sky_buffer,
            sky: [0.0; SKY_FLOATS],
            sky_built,
            sky_group,
            draws_buffer,
            draws_bytes: DRAWS_INITIAL * 4,
            bind_group: 0,
            models,
            shadow,
            shadows: false,
            ao,
            screen_ao: false,
            atmosphere,
            stars,
            clouds,
            mips,
            pack,
            moon,
            cases: [const { None }; SURFACES],
            order: Vec::new(),
        };
        r.box_groups(g);
        r
    }

    /// What a block's face is painted by, wherever it is drawn from
    /// (paint.wgsl): the frame, the sun's shadow, the texture pack, the occlusion
    /// and the air.
    fn lit_entries(&self, frame: u32) -> Vec<Entry> {
        let [map, sampler] = self.shadow.entries(1);
        let mut e = vec![
            buffer_entry(0, frame),
            map,
            sampler,
            view_entry(4, self.pack.view),
            buffer_entry(5, self.pack.table),
            sampler_entry(6, self.pack.sampler),
            self.ao.entry(7),
        ];
        e.extend(self.atmosphere.entries());
        e
    }

    /// The ghost boxes' group 0: what a block's face is painted by, and the draws.
    fn box_groups(&mut self, g: &Gpu) {
        release(self.bind_group);
        let mut e = self.lit_entries(self.frame_buffer);
        e.push(buffer_entry(3, self.draws_buffer));
        self.bind_group = g.bind_group(self.ghost.layouts[0], &e);
        let lit = self.lit_entries(self.frame_buffer);
        self.screen.lit(lit);
    }

    /// The occlusion map is a new object, so every lit pass's group 0 is remade.
    fn rebind(&mut self, g: &Gpu) {
        self.box_groups(g);
        self.models.rebind(
            g,
            self.frame_buffer,
            [self.ao.entry(7), self.ao.blank_entry(7)],
        );
    }

    /// `reach` how far the eye observes, in units; none without an eye.
    fn lay_out(&mut self, g: &Gpu, width: u32, height: u32, env: &[f32], reach: f32) {
        let s = stage_lane();
        let (mut m, mut proj) = (mat4::new(), mat4::new());
        proj.copy_from_slice(&s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16]);
        // The markers measure their size in view space.
        let b = &mut self.marker;
        put_mat(
            &mut b.view,
            0,
            s[STAGE_AT::MARKER_VIEW..STAGE_AT::MARKER_VIEW + 16]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
        );
        put_mat(&mut b.proj, 0, &proj);
        b.px_k = (s[STAGE_AT::VIEWPORT + 1]
            / (2.0 * libm::tan(s[STAGE_AT::FOV] * core::f64::consts::PI / 360.0)))
            as f32;

        let u = &mut self.frame;
        m.copy_from_slice(&s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16]);
        put_mat(u, VIEW_PROJ, &m);
        // Everything is drawn in the eye's frame; positions are taken against the eye
        // in f64 before they are narrowed.
        let eye_world = [s[STAGE_AT::EYE], s[STAGE_AT::EYE + 1], s[STAGE_AT::EYE + 2]];
        let eye = [0.0; 3];
        put_ray(&mut self.sky, sky::RAY, &m, &eye);
        self.clouds.lay_out(
            [eye_world[0], eye_world[1]],
            env,
            s[STAGE_AT::CLOUDS],
            reach * env[ENV_AIR + 1],
        );
        self.stars.lay_out(&m, env, width);
        self.sky[sky::POLE..sky::POLE + 3].copy_from_slice(&env[ENV_STARS + 8..ENV_STARS + 11]);
        self.sky[sky::MAPPED] = if self.moon.loaded { 1.0 } else { 0.0 };
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
        }
        u[EDGE_STRENGTH] = s[STAGE_AT::EDGE_STRENGTH] as f32;
        u[CELL] = s[STAGE_AT::CELL] as f32;
        u[LOW_SPEC] = s[STAGE_AT::LOW_SPEC] as f32;
        u[EDGE_WIDTH] = s[STAGE_AT::EDGE_WIDTH] as f32;
        u[RIM_INTENSITY] = s[STAGE_AT::RIM_INTENSITY] as f32;
        u[CORNER_AO] = s[STAGE_AT::CORNER_AO] as f32;
        u[VIEWPORT] = width as f32;
        u[VIEWPORT + 1] = height as f32;
        u[VIGNETTE] = 1.0;
        self.screen_ao = s[STAGE_AT::SCREEN_AO] != 0.0;
        u[SCREEN_AO] = if self.screen_ao { 1.0 } else { 0.0 };
        if self.screen_ao {
            self.ao.lay_out(&proj, width, height);
        }
        // The eye's own place on the map, Mercator's longitude and ordinate
        // (its units over the map's radius in them), and that radius.
        let lon = world::nearest(eye_world[0], 0.0) / MERCATOR_RADIUS_U;
        u[MAP..MAP + 3].copy_from_slice(&[
            lon as f32,
            (eye_world[1] / MERCATOR_RADIUS_U) as f32,
            MERCATOR_RADIUS_U as f32,
        ]);
        // The sun's map, as wide as the casters reach; none where the
        // shadows are off.
        let far = s[STAGE_AT::SHADOWS];
        self.shadows = far > 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        if self.shadows {
            // Up to a power of two, so the snap grid and the texel move only in steps;
            // the shadows fade by the casters' own reach.
            let half = libm::exp2(libm::ceil(libm::log2(far)));
            self.shadow
                .aim(&env[ENV_SUN..ENV_SUN + 3], eye, half, eye_world.map(|v| -v));
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
            u[SHADOW_TEXEL] = self.shadow.texel as f32;
            u[SHADOW_RIM] = (far / half) as f32;
        }

        self.atmosphere.lay_out(g, env);
    }

    /// A box is one instance shared by 36 vertices; a draw's first box is its first
    /// instance. Both vertex slots bind the same buffer; the record index rides
    /// on the first vertex.
    fn draw_boxes(&self, g: &Gpu, pass: u32, draws: &Draws) {
        let b = uploads();
        let mut bound = 0;
        for d in draws.of(PIPELINE_GHOST_BOX) {
            let (handle, box_cnt, first_box, uniform) = (d[0], d[1], d[2], d[3]);
            let buffer = b.get(handle).map(|(buffer, ..)| buffer);
            // A missing buffer is a fault in the list: skipped, it would draw
            // nothing with every gauge reading normal.
            let Some(buffer) = buffer else {
                xpute_kit::bug!(ENOTRECOVERABLE, handle)
            };
            if bound == 0 {
                g.set_pipeline(pass, self.ghost.pipeline);
                g.set_bind_group(pass, 0, self.bind_group, &[]);
            }
            if buffer != bound {
                g.set_vertex_buffer(pass, 0, buffer, 0, 0);
                g.set_vertex_buffer(pass, 1, buffer, 0, 0);
                bound = buffer;
            }
            g.draw(
                pass,
                BOX_VERTICES,
                box_cnt,
                uniform * BOX_VERTICES,
                first_box,
            );
        }
    }

    /// The world image's opaque models: the decor grown over the world. The
    /// impostors are the lit pass's alone. `bound` comes back as the group 0
    /// the pass has.
    fn draw_opaque(&mut self, g: &Gpu, pass: u32, b: &mut Uploads, stage: Stage) -> u32 {
        let mut bound = 0;
        self.decor.draw_grown(
            g,
            pass,
            self.screen.grown(),
            b,
            stage,
            (self.models.frame, &mut bound),
        );
        bound
    }

    /// A frame in two stages, each standing while what it reads stands:
    /// the cast's rays (stage 1, `RaysPass::prepare`); and the frame,
    /// everything the world and its light make off those rays, then over it
    /// what moves on its own — the actors, whose clips play, the clouds,
    /// which the wind carries, the puck, the ghosts, the markers, the gizmos
    /// (stage 2, `frame_read`). The rays hold the world as the cast left
    /// it, so the frame draws it again off them rather than off an image of
    /// it: that costs what laying an image would.
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
        self.skinned.set_skins(input.skins);
        read.read(input.skins);
        read.mark(self.pack.step(g, &self.mips, input.pack));
        read.mark(self.moon.step(g, &self.mips, input.moon));
        read.mark(self.atmosphere.earth.step(g, &self.mips, input.earth));
        let mut cast = input.cast;
        // What the frame cannot put off is recorded before what can wait:
        // the cast pass made first, then the squares' uploads, which take
        // only the room left.
        if cast.is_some() {
            self.screen.make(g, &mut self.pipelines);
        }
        self.screen.hand(g, &mut self.pipelines, &mut input.ground);
        let tiles = input.earth_tiles;
        read.mark(
            self.atmosphere
                .set_earth_tiles(g, tiles.window, tiles.layers, tiles.upload),
        );
        read.mark(
            self.marker
                .sync_sheet(g, &self.mips, input.sheet, input.text),
        );
        let (records, uniforms, uploads_moved) = take_lists(Some(g), input.lists);
        read.mark(uploads_moved);
        let uniforms_moved = read.read(uniforms);
        let lists_moved = read.read(records) | uniforms_moved;
        read.read(input.cases);

        targets(g, world);
        // How far the eye observes, as far as the clouds are drawn.
        let reach = cast
            .as_ref()
            .map_or(0.0, |c| f32::from_bits(c.eye[eye::REACH]));
        self.lay_out(g, world.width, world.height, input.environment, reach);
        // The light box the world is cast from, as the map is aimed; none
        // where no shadow is cast.
        if let Some(c) = cast.as_mut() {
            let sun = if self.shadows {
                self.shadow.sun
            } else {
                [0; eye::SUN_WORDS]
            };
            c.eye[eye::SUN..eye::SUN + eye::SUN_WORDS].copy_from_slice(&sun);
        }
        let ao_remade = self.screen_ao && self.ao.ensure(g, world.width, world.height);
        if ao_remade {
            self.rebind(g);
            read.mark(true);
        }
        let uniform_bytes = word_bytes(uniforms.len());
        let grew = uniform_bytes > self.draws_bytes;
        if grew {
            release(self.draws_buffer);
            self.draws_bytes = uniform_bytes * 2;
            self.draws_buffer = g.buffer(
                self.draws_bytes,
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            self.box_groups(g);
        }
        if !uniforms.is_empty() && (grew || uniforms_moved) {
            // Read where the draw list holds them, one record however many.
            g.upload_buffer(self.draws_buffer, 0, bytes(uniforms));
        }
        let mut order = core::mem::take(&mut self.order);
        let draws = Draws::sort(records.as_chunks::<DRAW_WORDS>().0, &mut order);
        // The camera gizmo listed tints the rays' blocks (screen_lit.wgsl `cell_share`).
        self.frame[GIZMO] = if draws.of(PIPELINE_CAMERA_GRID).len() > 0 {
            1.0
        } else {
            0.0
        };
        if read.read(&self.frame) {
            g.write_buffer(self.frame_buffer, 0, bytes(&self.frame));
            // The light's frame is of the same words.
            self.shadow.write(g);
        }
        if read.read(&self.sky) {
            g.write_buffer(self.sky_buffer, 0, bytes(&self.sky));
        }
        if read.read(self.stars.words()) {
            self.stars.write(g);
        }
        if self.screen_ao && read.read(self.ao.words()) {
            self.ao.write(g);
        }
        read.read(self.atmosphere.words());
        read.read(&[world.depth, world.rays, u32::from(self.screen_ao)]);
        let b = uploads();
        self.decor.prepare(g, &draws, uniforms, lists_moved, b);
        self.impostor
            .prepare(g, draws.of(PIPELINE_IMPOSTOR), uniforms, lists_moved, b);
        self.skinned
            .prepare(g, draws.of(PIPELINE_SKINNED), uniforms, lists_moved, b);
        self.puck.prepare(g, &draws, uniforms, lists_moved, b);
        self.marker.prepare(
            g,
            world.depth_view,
            (world.width, world.height),
            &draws,
            uniforms,
            b,
        );
        self.debug.prepare(g, &draws, uniforms, lists_moved);
        let meshes: [u32; GROUPS as usize] =
            core::array::from_fn(|k| self.decor.model_of(g, k as u32, b).map_or(0, |m| m.2));
        self.screen.prepare(
            g,
            cast,
            &meshes,
            (world.rays_view, [world.width, world.height]),
            (self.shadow.world, self.shadow.frame),
        );
        // The frame draws the decor grown, so it reads whether that is on:
        // decor turned off grows nothing and casts no ray again, yet the
        // frame stands without it.
        read.read(&[u32::from(self.screen.grown().is_some())]);
        self.prepare_cases(g, &draws, uniforms, input.cases, lists_moved);
        let q = self.query_set;
        // A query no pass wrote holds an earlier frame's value, so which were written is passed along.
        let mut stamps: [Stamp; GPU_SPAN_NAMES] = core::array::from_fn(|span| Stamp::of(q, span));
        // What outlives the frame is drawn and submitted ahead of it: from here on
        // the frame only describes itself.
        let ahead = g.encoder();
        let baked = self.impostor.bake(g, ahead, &self.decor, b);
        read.mark(baked);
        let gridded = self.screen.grids(g, ahead, &mut stamps[SPAN_FILL]);
        let folded = self.screen.fold(g, ahead, &mut stamps[SPAN_FILL]);
        let (tables, exposed) = self.atmosphere.draw(g, ahead, &mut stamps[SPAN_AIR]);
        read.mark(tables);
        let aired = self.atmosphere.froxel(
            g,
            ahead,
            [&self.frame, &self.sky],
            exposed,
            &mut stamps[SPAN_AIR],
        );
        read.mark(aired);
        if baked || gridded || folded || exposed || aired {
            g.submit(ahead);
        } else {
            release(ahead);
        }
        self.atmosphere.read_back(g);
        self.screen.read_back(g, &mut input.ground);
        self.screen.oracle_back(g);
        if read.read(self.clouds.words()) {
            self.clouds.write(g);
        }
        // The frame stands where all it draws from read as last time over
        // rays that stood: nothing is drawn, nothing submitted, and the
        // canvases keep the frame they show.
        if read.end()
            && self.screen.kind != FrameKind::cast
            && !self.screen.grows()
            && !self.screen.sun_casts()
            && !gridded
        {
            self.frame_read = read;
            let stat = self.stat(draws.records, world, input.counted, [0.0; GPU_SPAN_NAMES]);
            self.order = order;
            return (FrameKind::stood, stat);
        }

        // The frame proper changes nothing that outlives it, so one too large
        // for the stream is dropped whole and the screen keeps the last.
        frame_begin();
        let encoder = g.encoder();
        self.screen.dispatch(g, encoder, &mut stamps);
        // The sun's map: the world's depth from the sun, where the rays hold
        // the world it was cast with, then decor's and the actors' drawn over it.
        if self.shadows {
            let pass = self.shadow.begin(
                g,
                encoder,
                self.screen.kind != FrameKind::plain,
                &mut stamps[SPAN_SHADOW],
            );
            let mut bound = 0;
            let light = self.shadow.light_group;
            self.decor.draw_grown(
                g,
                pass,
                self.screen.grown(),
                b,
                Stage::Depth,
                (light, &mut bound),
            );
            self.decor
                .draw(g, pass, &draws, b, Stage::Depth, (light, &mut bound));
            self.skinned.draw(
                g,
                pass,
                draws.of(PIPELINE_SKINNED),
                b,
                Stage::Depth,
                (light, &mut bound),
            );
            g.end(pass);
        }
        let a = &mut self.ao_read;
        a.begin();
        a.read(&self.frame);
        a.read(self.ao.words());
        a.read(&[
            world.width,
            world.height,
            u32::from(self.screen_ao),
            u32::from(self.screen.grown().is_some()),
        ]);
        // The world's depth is the rays', so a cast that laid them again
        // lays the occlusion again.
        a.mark(ao_remade || self.screen.grows() || self.screen.kind == FrameKind::cast);
        let ao_moved = !a.end();
        if self.screen_ao && ao_moved {
            let pass = self.ao.begin_prepass(g, encoder, &mut stamps[SPAN_PREPASS]);
            self.screen.draw_depth(g, pass);
            self.draw_opaque(g, pass, b, Stage::Prepass);
            g.end(pass);
            self.ao.run(g, encoder, &mut stamps[SPAN_AO]);
        }
        let canvas_view = g.current(world.handle);
        let pass = g.pass(
            encoder,
            &pass_desc(
                canvas_view,
                Load::Clear,
                world.depth_view,
                Load::Clear,
                0.0,
                stamps[SPAN_WORLD].pass(),
            ),
        );
        self.draw_opaque(g, pass, b, Stage::Lit);
        let mut bound = self.screen.draw(g, pass);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor.draw_grown(
            g,
            pass,
            self.screen.grown(),
            GROUPS,
            (self.models.frame, &mut bound),
        );
        // At the far plane after the opaque world, so only empty pixels run the sky shader.
        g.set_pipeline(pass, self.sky_built.pipeline);
        g.set_bind_group(pass, 0, self.sky_group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.stars.draw(g, pass);
        // The actors, whose clips play, and the clouds the kernel lists, the
        // cumulus, which the wind carries: drawn over the world against its
        // depth, their occlusion open.
        let mut bound = 0;
        let over = (self.models.over, &mut bound);
        self.skinned.draw(
            g,
            pass,
            draws.of(PIPELINE_SKINNED),
            b,
            Stage::Lit,
            (over.0, &mut *over.1),
        );
        self.decor
            .draw(g, pass, &draws, b, Stage::Lit, (over.0, &mut *over.1));
        self.impostor
            .draw(g, pass, draws.of(PIPELINE_IMPOSTOR), b, over);
        self.clouds.draw(g, pass);
        self.puck.draw_beacon(g, pass, &draws);
        self.puck.draw_zone(g, pass, &draws, b);
        self.draw_boxes(g, pass, &draws);
        let mut bound = 0;
        self.decor
            .draw_ghosts(g, pass, &draws, b, (self.models.frame, &mut bound));
        g.end(pass);
        self.marker.draw(
            g,
            encoder,
            canvas_view,
            world.depth_view,
            &draws,
            b,
            &mut stamps[SPAN_MARKER],
        );
        self.debug.draw(
            g,
            encoder,
            canvas_view,
            world.depth_view,
            &draws,
            b,
            &mut stamps[SPAN_DEBUG],
        );
        self.draw_cases(
            g,
            encoder,
            Self::cases(&draws, input.cases),
            uniforms,
            uploads_moved || tables || exposed,
            &mut stamps[SPAN_CASE],
        );
        let written = stamps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.written())
            .fold(0, |m, (k, _)| m | 1 << k);
        let asked = g.submit_frame(encoder, written);
        release(canvas_view);
        // The read that landed is the one that was on its way; this frame's
        // takes its place where one was asked.
        let gpu_read = q != 0 && g.gpu_landed();
        let (gpu_spans, gpu_px) = if gpu_read {
            (core::array::from_fn(|k| g.gpu_ms(k as u32)), self.read_px)
        } else {
            ([0.0; GPU_SPAN_NAMES], 0.0)
        };
        if asked {
            self.read_px = f64::from(world.width) * f64::from(world.height);
        }
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
            self.screen.dropped();
            for case in self.cases.iter_mut().flatten() {
                case.read.forget();
            }
            read.forget();
            self.ao_read.forget();
        } else if self.screen.grows() {
            self.screen.drawn();
        }
        read.end();
        self.frame_read = read;
        let kind = if skipped {
            FrameKind::dropped
        } else {
            self.screen.kind
        };
        let mut stat = self.stat(draws.records, world, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
        stat.gpu_px = gpu_px;
        self.order = order;
        (kind, stat)
    }

    /// The cases drawn this frame: a surface still a case, its actor's model
    /// and its draw, found by the palette that is the case's alone, since
    /// another actor may play the same model.
    fn cases<'a, 'd>(
        draws: &'a Draws<'d>,
        cases: &'a [(u32, u32, Image)],
    ) -> impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])> + 'a {
        cases.iter().filter_map(move |&(id, palette, image)| {
            surface(id).filter(|s| s.role == surface_role::CASE)?;
            let own = draws.of(PIPELINE_SKINNED).find(|d| d[1] == palette)?;
            Some((id, own[2], image, own))
        })
    }

    /// Each case's targets, passes and skin, made here so drawing makes nothing
    /// that outlives its frame.
    fn prepare_cases(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        cases: &[(u32, u32, Image)],
        moved: bool,
    ) {
        for (id, model, image, own) in Self::cases(draws, cases) {
            let Some(s) = surface(id) else { continue };
            targets(g, s);
            let slot = handle_slot(id) as usize;
            if self.cases[slot].is_none() {
                self.cases[slot] = Some(Case::new(
                    g,
                    &mut self.pipelines,
                    &self.shadow,
                    &self.ao,
                    self.atmosphere.entries(),
                    self.align,
                ));
            }
            let case = self.cases[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            case.skinned.set_skins(&[(model, image)]);
            case.skinned
                .prepare(g, core::iter::once(own), uniforms, moved, uploads());
        }
    }

    /// Each case whose picture's makings moved, `moved` what moved without
    /// words: the models' uploads, the air's tables or its exposure.
    fn draw_cases<'d>(
        &mut self,
        g: &Gpu,
        encoder: u32,
        cases: impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])>,
        uniforms: &[f32],
        moved: bool,
        stamp: &mut Stamp,
    ) {
        for (id, model, image, own) in cases {
            let Some(s) = surface(id) else { continue };
            let Some(case) = self.cases[handle_slot(id) as usize].as_mut() else {
                continue;
            };
            case.draw(
                g, encoder, s, self.frame, own, uniforms, model, image, moved, stamp,
            );
        }
    }

    /// What the scheduler asked for: a draw passed over because its upload, skin or
    /// sprite is not on the device yet is still counted. The draws are counted
    /// only while `counted`, the profiler's, so a frame no one reads counts none.
    fn stat(
        &self,
        draws: &[[u32; DRAW_WORDS]],
        world: &Surface,
        counted: bool,
        gpu_spans: [f64; GPU_SPAN_NAMES],
    ) -> FrameStat {
        let b = uploads();
        let mut census: DrawCensus = [[0.0; 2]; DRAW_PIPELINES.len()];
        for d in draws.iter().filter(|_| counted) {
            let tris = match d[4] {
                PIPELINE_GHOST_BOX => d[1] * 12,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, index_cnt)| index_cnt / 3),
                PIPELINE_PUCK_ZONE => 12 * d[1],
                PIPELINE_PUCK_BEACON => 8,
                PIPELINE_MARKER => 2 * d[1],
                // Two a segment, and the faces'.
                PIPELINE_DEBUG => d[1] + d[2] / 3,
                PIPELINE_CAMERA_GRID => 1,
                _ => 0,
            };
            let c = &mut census[census_slot(d[4])];
            c[0] += 1.0;
            c[1] += tris as f64;
        }
        let px = world.width as f64 * world.height as f64;
        FrameStat {
            gpu_read: false,
            gpu_px: 0.0,
            uploads: b.count as f64,
            upload_bytes: b.bytes as f64,
            // The depth targets at the world's size: its own, one f32 a pixel, AO's and the markers' regions'.
            depth_bytes: px * 4.0 + self.ao.depth_bytes() + self.marker.depth_bytes(),
            pack_bytes: self.pack.bytes as f64,
            census,
            gpu_spans,
            oracle: self.screen.oracle_sums(),
        }
    }
}

/// A new size is applied once it has held SETTLE_TURNS turns; until then the
/// page scales the old buffer.
fn targets(g: &Gpu, s: &mut Surface) {
    let (width, height) = s.buffer_px();
    if !s.configured {
        g.configure(s.handle);
        s.configured = true;
        s.depth = 0;
    }
    let made = s.depth != 0;
    if made && s.width == width && s.height == height {
        s.waited = 0;
        return;
    }
    if made {
        if s.wanted != (width, height) {
            s.wanted = (width, height);
            s.waited = 0;
        }
        s.waited += 1;
        if s.waited < SETTLE_TURNS {
            REDRAW.set(true);
            return;
        }
    }
    s.waited = 0;
    g.resize(s.handle, width, height);
    for id in s.targets() {
        release(id);
    }
    let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
    s.depth = g.texture(width, height, 1, Format::depth32float, 1, usage);
    s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
    // The world's alone holds the cast's rays.
    (s.rays, s.rays_view) = (0, 0);
    if s.role == surface_role::WORLD {
        // Written by the cast, read by the screen pass.
        let rays = texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING;
        s.rays = g.texture(width, height, ray::WORDS, Format::r32uint, 1, rays);
        s.rays_view = g.view(s.rays, true, 0, 1, 0, ray::WORDS);
    }
    s.width = width;
    s.height = height;
}

/// A frame's cast handed and never drawn: the edits its decor named, held
/// for the next that is.
fn unconsumed(cast: &Option<Cast>) {
    let Some(d) = cast.as_ref().map(|c| &c.decor) else {
        return;
    };
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        r.screen.unconsumed(d.edits);
    }
}

/// Without a device or the world's canvas, the lists are recorded for the frame that has both.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == surface_role::WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), device(), world) else {
        unconsumed(&input.cast);
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), input.lists);
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: false,
            stat: None,
        };
    };
    // The surfaces would block: skip rather than wait.
    if !(input.admit)() {
        unconsumed(&input.cast);
        take_lists(Some(g), input.lists);
        return FrameOutput {
            kind: FrameKind::held,
            redraw: true,
            stat: None,
        };
    }
    let slot = RENDERER.get(|| None);
    let r = slot.get_or_insert_with(|| Renderer::new(g, device));
    uploads().restore(g);
    let (kind, stat) = r.frame(g, world, input);
    FrameOutput {
        kind,
        // An image landing is taken on a turn to come.
        redraw: *REDRAW.get(|| false) || crate::gpu::api::images_landing(),
        stat: Some(stat),
    }
}

/// Uploads listed with no device wait in the table for the next one.
/// The draws and their uniforms, and whether an upload was let go, made or
/// rewritten.
fn take_lists<'a>(g: Option<&Gpu>, lists: Lists<'a>) -> (&'a [u32], &'a [f32], bool) {
    let b = uploads();
    // Frees first, so the table never holds an upload beside its replacement.
    for &handle in lists.frees {
        b.remove(handle);
    }
    for u in lists.uploads.as_chunks::<UPLOAD_WORDS>().0 {
        b.insert(g, u[0], u[1], u[2], u[3], u[4]);
    }
    for r in lists.rewrites.as_chunks::<REWRITE_WORDS>().0 {
        b.rewrite(g, r[0], r[1], r[2], r[3]);
    }
    let moved = !(lists.frees.is_empty() && lists.uploads.is_empty() && lists.rewrites.is_empty());
    (lists.draws, lists.uniforms, moved)
}
