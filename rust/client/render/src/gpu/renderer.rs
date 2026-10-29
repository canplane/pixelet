// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec_core::eye;
use pxl_spec_core::param::sun;
use pxl_spec_core::ray;
use pxl_spec_core::scatter::GROUPS;
use pxl_spec_core::way;
use pxl_spec_core::window::{FRESH_WORDS, HANDED};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::api_spec::surface_role;
use crate::frame::draw::uniform_of;
use crate::frame::draw::{
    census_slot, DrawCensus, Lists, DRAW_PIPELINES, DRAW_WORDS, PIPELINE_CAMERA_GRID,
    PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_MARKER, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS,
    UPLOAD_WORDS,
};
use crate::frame::shader::record::{PIPELINE_SCREEN_CLEAR, PIPELINE_SKY};
use crate::frame::shader::word::frame::{
    CAMERA_POS, CELL, CORNER_AO, EDGE_STRENGTH, EDGE_WIDTH, GIZMO, JITTER, LIGHT_VIEW_PROJ,
    LOW_SPEC, MAP, RIM_INTENSITY, SCREEN, SCREEN_AO, SHADOW_RIM, SHADOW_STRENGTH, SHADOW_TEXEL,
    VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::{
    sky, BOX_VERTICES, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS,
};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::{ScreenAo, MAP_BINDING};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, device, frame_begin, frame_commit, gpu, on_device_lost,
    pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes, Device, Entry,
    Format, Gpu, Image, Load, RenderContext, Stamp, TrackedRenderPass,
};
use crate::gpu::atmosphere::{Atmosphere, Upload};
use crate::gpu::cloud::CloudPass;
use crate::gpu::finish::Finish;
use crate::gpu::grown::DecorFrame;
use crate::gpu::image_map::ImageMap;
use crate::gpu::mip::Mips;
use crate::gpu::pack::Pack;
use crate::gpu::pass::case::Case;
use crate::gpu::pass::debug::DebugPass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::marker::MarkerPass;
use crate::gpu::pass::model::{frame_layout, on_skins_lost, FrameGroups, Stage};
use crate::gpu::pass::puck::{PuckPass, BEACON_VERTICES};
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::ray::{RaysPass, RAY_SUMS};
use crate::gpu::read::Read;
use crate::gpu::resolve::Resolve;
use crate::gpu::shadow::{self, Shadow};
use crate::gpu::sheet::shelf::Sheet;
use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::star::StarPass;
use crate::gpu::surface::{
    on_surfaces_lost, surface, surfaces, Region, Surface, REGION_MARGIN, SETTLE_TURNS, SURFACES,
    WORLD_COLOR,
};
use crate::gpu::text::TextOf;
use crate::gpu::upload::{uploads, Draws, Uploads};
use crate::look::environment::{ENV_AIR, ENV_POLE, ENV_SUN};

const DRAWS_INIT: u32 = 256 * DRAW_FLOATS as u32;

const _: () = assert!(
    GPU_SPAN_NAMES <= 32,
    "a span a bit of the mask the host is told"
);

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
    pub ground: GroundBricks<'a>,
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
    /// The actors and the puck as drawn that changed since the last turn,
    /// where they were and where they are: a sphere each, units from the
    /// eye `[x, y, z]`, its radius, and 1 where it casts a shadow that moves
    /// with it. Where nothing else moved, the frame draws again alone the
    /// part of the world and of the sun's map they cover and shade
    /// (`regions`).
    pub moved: &'a [[f32; 5]],
    /// Whether what the kernel animates stepped this turn: it wakes the turn
    /// at its own steps, so the resolve asks no frame after it.
    pub animated: bool,
}

/// How far along the sun's ray, away from it, an actor's shadow is taken
/// to fall: in its radii over the sine of the sun's height.
const SHADOW_RADII: f64 = 2.0;

/// The texels beside a caster's that it changes, in the sun's map and,
/// through a receiver's taps, on the screen: the map's filter, a slope's
/// push and the normal's offset, with room (shadow.wgsl `sun_lit`).
const SHADOW_MARGIN: f64 = 4.0;

/// Where spheres in the eye's frame, `[x, y, z]` and a radius, fall
/// through `m`: the clip `[x0, y0, x1, y1]` their boxes cover; none where
/// one stands about or behind the eye.
fn clip_of(m: &Mat4, spheres: impl Iterator<Item = (V3, f64)>) -> Option<[f32; 4]> {
    let mut clip: Option<[f64; 4]> = None;
    for (p, r) in spheres {
        if v3::len(&p) <= r {
            return None;
        }
        for k in 0..8 {
            let c: V3 = core::array::from_fn(|a| p[a] + if k >> a & 1 == 1 { r } else { -r });
            let w = m[3] * c[0] + m[7] * c[1] + m[11] * c[2] + m[15];
            if w.is_nan() || w <= 0.0 {
                return None;
            }
            let x = (m[0] * c[0] + m[4] * c[1] + m[8] * c[2] + m[12]) / w;
            let y = (m[1] * c[0] + m[5] * c[1] + m[9] * c[2] + m[13]) / w;
            let b = clip.get_or_insert([x, y, x, y]);
            *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    clip.map(|b| b.map(|v| v.clamp(-1.0, 1.0) as f32))
}

/// The world's draws a region does not stand for (`FrameInput::moved`):
/// any of theirs that moved draws the frame whole.
const BESIDE: [u32; 5] = [
    PIPELINE_DECOR,
    PIPELINE_GHOST_BOX,
    PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR,
    PIPELINE_CAMERA_GRID,
];

/// The earth layer's tiles as the GPU reads them: the floor's window, its
/// south-west tile first, and per tile its layer plus one; `upload` hands each
/// tile that has come to the writer while the writer takes it.
pub struct EarthTiles<'a> {
    pub window: [i32; 2],
    pub layers: &'a [u32],
    pub upload: &'a mut Upload,
}

/// The ground's windows as the GPU holds them (pxl-spec-core `window`,
/// their bricks in `SLOTS` slots): the bricks to hand it, and where
/// their cells come back.
pub struct GroundBricks<'a> {
    /// Hands each brick whose cells the GPU is to draw to the writer.
    pub upload: &'a mut TerrainUpload,
    /// Told each brick whose cells came back, and the fill they are of
    /// (`CellsBack::ticket`).
    pub landed: &'a mut CellsLanded<'a>,
    /// The ways' pool's families at first (pxl-kernel domain/scene/param.rs
    /// `WAYS_POOL_FIRST_BYTES`): it doubles each time a build runs out, up
    /// to `way::tree::POOL_FAMILIES` (`GroundBuffers::grow`).
    pub pool: u32,
    /// The families each handed brick's scratch holds a tree of at first
    /// (pxl-kernel domain/scene/param.rs `WAYS_SCRATCH_FIRST_BYTES`): it
    /// doubles each time a build outgrows it, up to
    /// `way::tree::BRICK_FAMILIES` (`GroundBuffers::grow_scratch`).
    pub scratch: u32,
    /// A read's records' room at first, in bytes: every read's is doubled
    /// past a brick's records that outgrew it, up to
    /// `way::tree::RECORDS_BYTES` (`GroundBuffers::record_bytes`).
    pub records: u32,
}

/// How a brick's fill landed: its cells are back in every case.
pub enum Landing<'a> {
    /// Its ways' tree built: the families it holds, their count first, a
    /// record each (pxl-spec-core `way::tree::lay`).
    Built(&'a [u32]),
    /// To be filled again, for want of room the renderer grows: its tree not
    /// built, the pool or the scratch it was built in having run out
    /// (`Short`), or built but its records past its read's room, the tree
    /// the refill frees.
    Again,
    /// Its tree refused, the pool or the scratch at its most having run out,
    /// or the brick past its own (`way::tree::past`): the brick stays
    /// absent.
    Refused,
}

/// The read-backs a brick's cells and its ways' records come back in
/// (gpu/ray/ground.rs `GroundBuffers::read_back`): two frames' fills, so one
/// frame's mappings are still landing while the next is copied.
pub const READS: u32 = 2 * HANDED;

/// The pool's records the renderer keeps on the heap at their worst, in
/// the buddy's blocks: every read's room at a brick's most
/// (`GroundBuffers::room`), each a power of two as the heap gives it.
pub const RECORD_ROOMS_BYTES: u64 =
    READS as u64 * (way::tree::RECORDS_BYTES as u64).next_power_of_two();

/// A brick whose cells came back, the fill they are of
/// (`CellsBack::ticket`), and how its ways' tree landed; what is told may
/// borrow its caller's for the frame.
pub type CellsLanded<'a> = dyn FnMut(u32, u32, Landing<'_>) + 'a;

/// The GPU's cast this frame (frame/shader/cast.gen.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-spec-core `eye`; pxl-kernel domain/view/lens.rs `gpu_cast`,
    /// domain/scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// The windows' table (pxl-spec-core `window::Entry`; pxl-kernel
    /// domain/layer/terrain/window.rs `table`), four words a row: every window's
    /// entries, the eye's first, from the top's, each the brick it wants, X
    /// and Y, the slot holding it plus one while present (`Entry::slot`)
    /// and the fill it is on its way from; then each window's first brick
    /// (`window::ORIGIN_AT`).
    pub window: &'a [i32],
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row (pxl-kernel domain/scene/cast.rs `shards`); read where it lies,
    /// unwritten until the page replays the turn.
    pub shards: &'a [[i32; 4]],
    pub overwrites: Overwrites<'a>,
    /// The decor the GPU grows and draws over the same world (gpu/grown.rs).
    pub decor: DecorFrame<'a>,
    /// Where each actor drawn stands, units from the eye, as its draw is
    /// placed (frame/draw.rs `Lists::stands`): receivers of the shadow map as
    /// decor is (pxl-spec-core `sun::ACTORS_AT`).
    pub actors: &'a [[f32; 3]],
    /// Whether what the eye's cast and the sun's walked and took is counted
    /// (gpu/ray/tally.rs).
    pub ray_counts: bool,
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

/// Where a brick's cells, which the GPU draws, come back to in the
/// kernel's memory: its columns, levels, laid cells and root
/// (pxl-spec-core `window::BACK_WORDS`); and the fill's ticket, never 0,
/// which the kernel is told once this fill's are in.
#[derive(Clone, Copy, Default)]
pub struct CellsBack {
    pub cells: u32,
    pub ticket: u32,
}

/// A brick handed to the GPU's fill, the record the ground's windows hand
/// on (pxl-kernel domain/layer/terrain/window.rs `fill`): its words
/// (pxl-spec-core `window::fresh`), its four looks a cell, its tile's
/// padded samples, its lines as far as they are written
/// (`window::LINES_AT`), and where its cells come back to.
pub struct FreshBrick<'a> {
    pub fresh: [u32; FRESH_WORDS as usize],
    pub looks: &'a [u8],
    pub samples: &'a [i16],
    pub lines: &'a [u32],
    pub back: CellsBack,
}

/// Takes a brick while the turn's stream has room for it.
pub type TerrainWrite<'a> = dyn FnMut(FreshBrick<'_>) -> bool + 'a;

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
    /// read's, `gpu_px` the world's pixels the frame it timed drew, what the
    /// spans were measured over, and `gpu_frame` that frame's number (`frame`
    /// as it was); else they are 0, as nothing was measured with this frame.
    pub gpu_read: bool,
    pub gpu_px: f64,
    pub gpu_frame: f64,
    /// This frame's number, from 1, a frame the renderer encoded each; 0
    /// where it stood and drew nothing.
    pub frame: f64,
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
    /// The eye's cast's and the sun's counts, summed since boot
    /// (gpu/ray/tally.rs `RAY_SUMS`).
    pub rays: [f64; RAY_SUMS],
}

/// A frame's GPU time: its spans' sum, the passes timed. The span from the
/// first to the last would count what lies between submits.
pub fn gpu_ms(spans: &[f64]) -> f64 {
    spans.iter().sum()
}

/// The passes drawn at the world's own pixels, whose cost goes with them:
/// its depth prepass, its occlusion, its drawing, its gathering over frames
/// and its cast. The air's
/// tables, the fill and the decor's growth, the thumbnails, markers and debug
/// cost what they cost at any resolution.
const PER_PX: [GpuSpanName; 11] = [
    GpuSpanName::prepass,
    GpuSpanName::ao,
    GpuSpanName::world,
    GpuSpanName::resolve,
    GpuSpanName::cast,
    GpuSpanName::beam,
    GpuSpanName::cell,
    GpuSpanName::sample,
    GpuSpanName::sraster,
    GpuSpanName::px,
    GpuSpanName::miss,
];

/// A frame's GPU time in the passes drawn at the world's pixels (`PER_PX`):
/// what its drawn resolution can buy back.
pub fn px_ms(spans: &[f64]) -> f64 {
    PER_PX.iter().map(|&s| spans[s as usize]).sum()
}

/// The ray under NDC (x, y) is `x·col0 + y·col1 + col2`, the screen's lens
/// off the view's planes (pxl-spec `Frustum::lens`, as the cast's eye takes
/// it): its right and up by their tangents, and its forward, each taken in
/// f64 before it is narrowed. Zero without a screen.
fn put_ray(out: &mut [f32], at: usize, view_proj: &Mat4) {
    let mut planes = Frustum::default();
    mat4::frustum_planes(&mut planes.planes, view_proj);
    let columns = planes.lens().map_or([[0.0; 3]; 3], |l| {
        [
            l.right.map(|c| c * l.tan[0]),
            l.up.map(|c| c * l.tan[1]),
            l.forward,
        ]
    });
    for (k, column) in columns.iter().enumerate() {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3].iter_mut().zip(column) {
            *o = *e as f32;
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

/// The frames encoded since boot, drawn or dropped, the last one's number
/// (`FrameStat::frame`): beside the renderer, so a lost device's next
/// renderer numbers on.
static DRAWN_FRAMES: Global<f64> = Global::new();

pub fn on_gpu_lost() {
    RENDERER.set(None);
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
    /// The world's pixels and the number of the frame whose spans' read is
    /// on its way: one read at most is (`Gpu::submit_frame`).
    read_px: f64,
    read_frame: f64,
    pipelines: Pipelines,
    frame_buf: u32,
    frame: Frame,
    sky_buf: u32,
    sky: [f32; SKY_FLOATS],
    sky_built: Built,
    sky_group: u32,
    ghost: Built,
    draws_buf: u32,
    draws_bytes: u32,
    bind_group: u32,
    models: FrameGroups,
    shadow: Shadow,
    /// Whether the shadow map is cast and drawn this frame (`STAGE_AT::SHADOWS`).
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
    finish: Finish,
    resolve: Resolve,
    debug: DebugPass,
    screen: RaysPass,
    /// The frame's whole input as the last frame drew it (stage 2): the
    /// frame's words but the jitter, the sky's and the stars' words, the
    /// air's, the surface's textures, the draws and the uploads they read,
    /// the markers' sheet, the clouds' words and the cases, and what came or
    /// was drawn again without words.
    frame_read: Read,
    /// The jitter in the frame's words as the GPU holds them (`JITTER`).
    jitter_written: [f32; 2],
    /// The lists as the last frame drew them, apart from the rest of its
    /// input: their records, uniforms and uploads (`listed`), and the
    /// records and uniforms of the draws a region does not stand for, with
    /// whether an upload they read moved (`beside`, `BESIDE`).
    listed: Read,
    beside: Read,
    /// The region a frame draws again cleared, under its scissor.
    clear: Built,
    /// The eye's view-projection as `lay_out` took it, which the moved
    /// spheres are seen through (`regions`).
    view_proj: Mat4,
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
        let frame_buf = g.buf(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let sky_buf = g.buf(
            word_bytes(SKY_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buf);
        let atmosphere = Atmosphere::new(g, &mut pipelines, frame_buf, sky_buf);
        let air = atmosphere.entries();
        let stars = StarPass::new(g, &mut pipelines, frame_buf, air);
        let clouds = CloudPass::new(g, &mut pipelines, frame_buf, air);
        let mips = Mips::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let pack = Pack::new(g);
        let sky_built = pipelines.build(g, PIPELINE_SKY, WORLD_COLOR);
        let moon = ImageMap::new(g, MOON_TEXELS, MOON_TEXELS);
        let mut sky_entries = vec![
            buf_entry(0, frame_buf),
            buf_entry(19, sky_buf),
            view_entry(2, moon.view),
            sampler_entry(3, moon.sampler),
        ];
        sky_entries.extend(air);
        let sky_group = g.bind_group(sky_built.layouts[0], &sky_entries);
        let draws_buf = g.buf(
            DRAWS_INIT * 4,
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let shadow = Shadow::new(g, &mut pipelines);
        let models = FrameGroups::new(
            g,
            frame_layout(g, &mut pipelines),
            frame_buf,
            shadow.entries(1),
            [ao.blank_entry(MAP_BINDING); 2],
            air,
        );
        let query_set = if device.timestamps { g.spans() } else { 0 };
        let mut r = Renderer {
            align: device.align,
            query_set,
            read_px: 0.0,
            read_frame: 0.0,
            ghost: pipelines.build(g, PIPELINE_GHOST_BOX, WORLD_COLOR),
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned: SkinnedPass::new(g, &mut pipelines, device.align),
            puck: PuckPass::new(g, &mut pipelines, frame_buf, air, device.align),
            marker: MarkerPass::new(g, &mut pipelines, frame_buf, device.align),
            finish: Finish::new(g, &mut pipelines),
            resolve: Resolve::new(g, &mut pipelines),
            debug: DebugPass::new(g, &mut pipelines, frame_buf, sky_buf, device.align),
            screen: RaysPass::new(g, &mut pipelines),
            frame_read: Read::default(),
            jitter_written: [f32::NAN; 2],
            listed: Read::default(),
            beside: Read::default(),
            clear: pipelines.build(g, PIPELINE_SCREEN_CLEAR, WORLD_COLOR),
            view_proj: mat4::new(),
            ao_read: Read::default(),
            pipelines,
            frame_buf,
            frame: [0.0; FRAME_FLOATS],
            sky_buf,
            sky: [0.0; SKY_FLOATS],
            sky_built,
            sky_group,
            draws_buf,
            draws_bytes: DRAWS_INIT * 4,
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

    /// What a cell's face is painted by, wherever it is drawn from
    /// (paint.wgsl): the frame, the sun's shadow, the texture pack, the occlusion
    /// and the air.
    fn lit_entries(&self, frame: u32) -> Vec<Entry> {
        let [map, sampler] = self.shadow.entries(1);
        let mut e = vec![
            buf_entry(0, frame),
            map,
            sampler,
            view_entry(4, self.pack.view),
            buf_entry(5, self.pack.table),
            sampler_entry(6, self.pack.sampler),
            self.ao.entry(MAP_BINDING),
        ];
        e.extend(self.atmosphere.entries());
        e
    }

    /// The ghost boxes' group 0: what a cell's face is painted by, and the draws.
    fn box_groups(&mut self, g: &Gpu) {
        release(self.bind_group);
        let mut e = self.lit_entries(self.frame_buf);
        e.push(buf_entry(3, self.draws_buf));
        self.bind_group = g.bind_group(self.ghost.layouts[0], &e);
        let lit = self.lit_entries(self.frame_buf);
        self.screen.set_lit(lit);
    }

    /// The occlusion map is a new object, so every lit pass's group 0 is remade.
    fn rebind(&mut self, g: &Gpu) {
        self.box_groups(g);
        self.models.rebind(
            g,
            self.frame_buf,
            [self.ao.entry(MAP_BINDING), self.ao.blank_entry(MAP_BINDING)],
        );
    }

    /// `reach` how far the eye draws, in units; none without an eye.
    fn lay_out(
        &mut self,
        g: &Gpu,
        width: u32,
        height: u32,
        screen: (u32, u32),
        env: &[f32],
        reach: f32,
    ) {
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
        b.px_per_unit = s[STAGE_AT::PX_PER_UNIT] as f32;

        let u = &mut self.frame;
        m.copy_from_slice(&s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16]);
        put_mat(u, VIEW_PROJ, &m);
        self.view_proj = m;
        // Everything is drawn in the eye's frame; positions are taken against the eye
        // in f64 before they are narrowed.
        let eye_world = [s[STAGE_AT::EYE], s[STAGE_AT::EYE + 1], s[STAGE_AT::EYE + 2]];
        self.resolve.lay_out(&m, eye_world);
        let eye = [0.0; 3];
        put_ray(&mut self.sky, sky::RAY, &m);
        self.clouds.lay_out(
            [eye_world[0], eye_world[1]],
            env,
            s[STAGE_AT::CLOUDS],
            reach * env[ENV_AIR + 1],
        );
        self.stars.lay_out(&m, env, width);
        self.sky[sky::POLE..sky::POLE + 3].copy_from_slice(&env[ENV_POLE..ENV_POLE + 3]);
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
        u[SCREEN] = screen.0 as f32;
        u[SCREEN + 1] = screen.1 as f32;
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
        // The shadow map, as wide as the casters reach; none where the
        // shadows are off.
        let far = s[STAGE_AT::SHADOWS];
        self.shadows = far > 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        if self.shadows {
            // Up to a power of two, so the snap grid and the texel move only in steps;
            // the shadows fade by the casters' own reach.
            let half = libm::exp2(libm::ceil(libm::log2(far)));
            let rim = (far / half) as f32;
            self.shadow.aim(
                &env[ENV_SUN..ENV_SUN + 3],
                eye,
                half,
                rim,
                eye_world.map(|v| -v),
            );
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
            u[SHADOW_TEXEL] = self.shadow.texel as f32;
            u[SHADOW_RIM] = rim;
        }

        self.atmosphere.lay_out(g, env, reach);
    }

    /// A box is one instance shared by `BOX_VERTICES`; a draw's first box is its first
    /// instance. Both vertex slots bind the same buffer; the record index rides
    /// on the first vertex.
    fn draw_boxes(&self, pass: &mut TrackedRenderPass, draws: &Draws) {
        let b = uploads();
        for d in draws.of(PIPELINE_GHOST_BOX) {
            let (handle, box_cnt, first_box, uniform) = (d[0], d[1], d[2], d[3]);
            let buf = b.get(handle).map(|(buf, ..)| buf);
            // A missing buffer is a fault in the list: skipped, it would draw
            // nothing with every gauge reading normal.
            let Some(buf) = buf else {
                xpute_kit::bug!(ENOTRECOVERABLE, handle)
            };
            pass.set_pipeline(self.ghost.pipeline);
            pass.set_bind_group(0, self.bind_group, &[]);
            pass.set_vertex_buf(0, buf, 0, 0);
            pass.set_vertex_buf(1, buf, 0, 0);
            pass.draw(BOX_VERTICES, box_cnt, uniform * BOX_VERTICES, first_box);
        }
    }

    /// Where the `moved` spheres fall: on the screen, the drawn pixels of a
    /// world `width` by `height` they cover and their shadows shade, the
    /// shadows grown by as far as a receiver's taps spread (`Region`); and
    /// in the sun's map, the texels the ones that cast cover, `[x, y,
    /// width, height]`, none where none casts. None at all where nothing
    /// moved, or where they cannot be bounded: a shadow cast from a sun on or
    /// under the horizon, or a sphere about or behind the eye. A shadow is
    /// taken to fall on ground about level with the caster (`SHADOW_RADII`),
    /// at any height of the sun. Ground falling away from the sun more
    /// steeply, or a drop, takes the shadow's end past the region, where it
    /// stays until a frame is drawn whole; how steeply it may fall shrinks
    /// with the sun, about 10° at a sun 14.5° up and 7° at 10°. A region past
    /// most of the screen is drawn whole (`Region::of`).
    fn regions(
        &self,
        moved: &[[f32; 5]],
        width: u32,
        height: u32,
    ) -> Option<(Region, Option<[u32; 4]>)> {
        if moved.is_empty() {
            return None;
        }
        let sun = self.shadow.cast_from.filter(|_| self.shadows);
        // A receiver's taps, the filter's and the normal's offset, in units.
        let spread = (f64::from(sun::SPREAD) + SHADOW_MARGIN) * self.shadow.texel;
        let casts = |s: &[f32; 5]| s[4] != 0.0 && sun.is_some();
        if moved.iter().any(casts) && sun.is_some_and(|s| s[2] <= 0.0) {
            return None;
        }
        let sphere = |s: &[f32; 5]| ([s[0], s[1], s[2]].map(f64::from), f64::from(s[3]));
        // Each, and the shadow of each that casts along the ground away
        // from the sun.
        let shaded = moved.iter().flat_map(|s| {
            let (p, r) = sphere(s);
            let cast = sun.filter(|_| casts(s)).map(|sun| {
                let reach = SHADOW_RADII * r / sun[2];
                let at: V3 = core::array::from_fn(|k| p[k] - sun[k] * reach);
                (at, r + reach / 2.0 + spread)
            });
            [Some((p, r + spread * f64::from(u8::from(casts(s))))), cast]
                .into_iter()
                .flatten()
        });
        let region = Region::of(
            clip_of(&self.view_proj, shaded)?,
            width,
            height,
            REGION_MARGIN,
        )?;
        let in_map = clip_of(
            &self.shadow.light_view_proj,
            moved.iter().filter(|s| casts(s)).map(sphere),
        )
        .map(|[x0, y0, x1, y1]| {
            let n = f64::from(shadow::SIZE);
            let at = |v: f32| f64::from(v) * 0.5 + 0.5;
            let lo = [
                at(x0) * n - SHADOW_MARGIN,
                (1.0 - at(y1)) * n - SHADOW_MARGIN,
            ];
            let hi = [
                at(x1) * n + SHADOW_MARGIN,
                (1.0 - at(y0)) * n + SHADOW_MARGIN,
            ];
            let lo = lo.map(|v| v.clamp(0.0, n) as u32);
            let hi = hi.map(|v| libm::ceil(v.clamp(0.0, n)) as u32);
            [lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]]
        });
        Some((region, in_map))
    }

    /// Whether a draw a region does not stand for moved since the last
    /// frame (`BESIDE`): its record or uniform, or an upload it reads
    /// (`touched`, the handles this frame's lists upload, rewrite or free).
    fn beside_moved(&mut self, records: &[u32], uniforms: &[f32], touched: &[u32]) -> bool {
        let r = &mut self.beside;
        r.begin();
        for d in records.as_chunks::<DRAW_WORDS>().0 {
            if BESIDE.contains(&d[4]) {
                r.read(d);
                r.read(uniform_of(uniforms, d));
                r.mark(touched.contains(&d[0]));
            }
        }
        !r.end()
    }

    /// The world image's opaque models: the decor grown over the world. The
    /// impostors are the lit pass's alone.
    fn draw_opaque(&mut self, pass: &mut TrackedRenderPass, b: &mut Uploads, stage: Stage) {
        self.decor
            .draw_grown(pass, self.screen.grown(), b, stage, self.models.frame);
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
        // the cast pass made first, then the bricks' uploads, which take
        // only the room left.
        if cast.is_some() {
            self.screen.make(g, &mut self.pipelines);
            self.screen.sun_made(g, self.shadows);
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
        let lists = input.lists;
        let touched: Vec<u32> = lists
            .uploads
            .as_chunks::<UPLOAD_WORDS>()
            .0
            .iter()
            .map(|u| u[0])
            .chain(
                lists
                    .rewrites
                    .as_chunks::<REWRITE_WORDS>()
                    .0
                    .iter()
                    .map(|r| r[0]),
            )
            .chain(lists.frees.iter().copied())
            .collect();
        let (records, uniforms, uploads_moved) = take_lists(Some(g), lists);
        let l = &mut self.listed;
        l.begin();
        l.mark(uploads_moved);
        let uniforms_moved = l.read(uniforms);
        let lists_moved = l.read(records) | uniforms_moved;
        let listed_same = l.end();
        let beside_moved = self.beside_moved(records, uniforms, &touched);
        read.read(input.cases);

        targets(g, world);
        let animated = input.animated;
        // How far the eye draws, as far as the clouds are drawn.
        let reach = cast
            .as_ref()
            .map_or(0.0, |c| f32::from_bits(c.eye[eye::REACH]));
        self.lay_out(
            g,
            world.width,
            world.height,
            world.screen,
            input.environment,
            reach,
        );
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
        // Where each pixel's ray goes through it this frame, the cast's and
        // what is drawn over the world alike (gpu/resolve.rs), in clip units
        // in the frame's words.
        let j = self.resolve.jitter();
        if let Some(c) = cast.as_mut() {
            c.eye[eye::JITTER] = j[0].to_bits();
            c.eye[eye::JITTER + 1] = j[1].to_bits();
        }
        self.frame[JITTER] = 2.0 * j[0] / world.width as f32;
        self.frame[JITTER + 1] = -2.0 * j[1] / world.height as f32;
        // Where what the kernel animates moved, on the screen and in the
        // sun's map, laid out as this frame's eye and light see it.
        let (region, sun_rect) = self
            .regions(input.moved, world.width, world.height)
            .map_or((None, None), |(r, s)| (Some(r), s));
        let ao_remade = self.screen_ao && self.ao.ensure(g, world.width, world.height);
        if ao_remade {
            self.rebind(g);
            read.mark(true);
        }
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
        }
        if !uniforms.is_empty() && (grew || uniforms_moved) {
            // Read where the draw list holds them, one record however many.
            g.upload_buf(self.draws_buf, 0, bytes(uniforms));
        }
        let mut order = core::mem::take(&mut self.order);
        let draws = Draws::sort(records.as_chunks::<DRAW_WORDS>().0, &mut order);
        // The camera gizmo listed tints the rays' cells (screen_lit.wgsl `cell_share`).
        self.frame[GIZMO] = if draws.of(PIPELINE_CAMERA_GRID).len() > 0 {
            1.0
        } else {
            0.0
        };
        // The jitter apart: where only it moved, the frame draws its pixels
        // again and stands once it holds (gpu/resolve.rs).
        let jitter = [self.frame[JITTER], self.frame[JITTER + 1]];
        let words_moved = read.read(&self.frame[..JITTER]);
        if words_moved || jitter != self.jitter_written {
            g.write_buf(self.frame_buf, 0, bytes(&self.frame));
            self.jitter_written = jitter;
        }
        if words_moved {
            // The light's frame is of the same words.
            self.shadow.write(g);
        }
        if read.read(&self.sky) {
            g.write_buf(self.sky_buf, 0, bytes(&self.sky));
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
            world.screen_depth_view,
            world.screen,
            &draws,
            uniforms,
            b,
        );
        self.debug.prepare(g, &draws, uniforms, lists_moved);
        let meshes: [u32; GROUPS as usize] =
            core::array::from_fn(|k| self.decor.model_of(g, k as u32, b).map_or(0, |m| m.2));
        // A region drawn again alone, where nothing but what it bounds moved
        // so far; the cast takes it, its pixels cast there alone.
        let region = region.filter(|_| read.same() && !beside_moved);
        self.screen.prepare(
            g,
            cast,
            &meshes,
            (world.rays_view, [world.width, world.height]),
            (self.shadow.world, self.shadow.frame, self.shadow.starts()),
            region,
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
        let mut ctx = RenderContext::new(g, ahead, &mut stamps);
        let gridded = self.screen.grids(&mut ctx);
        let folded = self.screen.fold(&mut ctx);
        let (tables, exposed) = self.atmosphere.draw(&mut ctx);
        read.mark(tables);
        let aired = self
            .atmosphere
            .froxel(&mut ctx, [&self.frame[..JITTER], &self.sky], exposed);
        read.mark(aired);
        if baked || gridded || folded || exposed || aired {
            g.submit(ahead);
        } else {
            release(ahead);
        }
        self.atmosphere.read_back(g);
        self.screen.read_back(g, &mut input.ground);
        self.screen.tally_back(g);
        if read.read(self.clouds.words()) {
            self.clouds.write(g);
        }
        // The world as it stood: all the frame draws from read as last time
        // but its lists and its jitter, over rays cast again at most at
        // their pixels.
        let stands =
            read.end() && self.screen.kind != FrameKind::cast && !self.screen.grows() && !gridded;
        // Nothing moved but the jitter.
        let quiet = stands && listed_same;
        // Only what the region bounds moved: it alone is drawn again. Else
        // the frame is drawn whole, its pixels cast whole.
        let region = region.filter(|_| stands);
        if region.is_none() {
            self.screen.whole(g);
        }
        // The frame stands where nothing moved, the jitter neither: nothing
        // is drawn, nothing submitted, and the canvases keep the frame they
        // show.
        if quiet && !self.screen.casts_px() && !self.screen.sun_casts() {
            self.frame_read = read;
            let mut stat = self.stat(draws.records, world, input.counted, [0.0; GPU_SPAN_NAMES]);
            // Nothing drawn, so no number a read could name.
            stat.frame = 0.0;
            self.order = order;
            return (FrameKind::stood, stat);
        }

        // The frame proper changes nothing that outlives it, so one too large
        // for the stream is dropped whole and the screen keeps the last.
        frame_begin();
        let encoder = g.encoder();
        let mut ctx = RenderContext::new(g, encoder, &mut stamps);
        self.screen.dispatch(&mut ctx);
        // The shadow map: the world's depth from the sun, where the rays hold
        // the world it was cast with, then decor's and the actors' drawn over it.
        // The sun's map where it moved: whole, but where a region alone is
        // drawn again over the world the sun's cast left as it was, the part
        // the casters that moved cover, or none of it.
        let cast = self.screen.kind != FrameKind::plain;
        let map = match (region, sun_rect) {
            (Some(_), _) if !cast || self.screen.sun_casts() => Some(None),
            (Some(_), Some(rect)) => Some(Some(rect)),
            (Some(_), None) => None,
            (None, _) => Some(None),
        };
        if let Some(rect) = map.filter(|_| self.shadows) {
            let mut pass = self.shadow.begin(&mut ctx, cast, rect);
            let light = self.shadow.light_group;
            self.decor
                .draw_grown(&mut pass, self.screen.grown(), b, Stage::Depth, light);
            self.decor.draw(&mut pass, &draws, b, Stage::Depth, light);
            self.skinned.draw(
                &mut pass,
                draws.of(PIPELINE_SKINNED),
                b,
                Stage::Depth,
                light,
            );
            pass.end();
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
        a.mark(ao_remade || self.screen.grows() || self.screen.casts_px());
        let ao_moved = !a.end();
        // A region drawn again alone moves no depth the occlusion reads: its
        // prepass draws the rays and the grown decor, never what the kernel
        // animates (gpu/pass/model.rs, drawn over it), so it stands.
        if self.screen_ao && ao_moved && region.is_none() {
            let mut pass = self.ao.begin_prepass(&mut ctx);
            self.screen.draw_depth(&mut pass);
            self.draw_opaque(&mut pass, b, Stage::Prepass);
            pass.end();
            self.ao.run(&mut ctx);
        }
        let canvas_view = g.cur(world.handle);
        // A region drawn again alone keeps the rest as the last frame left
        // it, the region cleared under the scissor first.
        let load = if region.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let pass = g.pass(
            encoder,
            &pass_desc(
                world.color_view,
                load,
                world.depth_view,
                load,
                0.0,
                ctx.stamp(GpuSpanName::world).pass(),
            ),
        );
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(r) = region {
            pass.set_scissor(r.rect());
            pass.set_pipeline(self.clear.pipeline);
            pass.draw(3, 1, 0, 0);
        }
        self.draw_opaque(&mut pass, b, Stage::Lit);
        self.screen.draw(&mut pass);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor
            .draw_grown(&mut pass, self.screen.grown(), GROUPS, self.models.frame);
        // At the far plane after the opaque world, so only empty pixels run the sky shader.
        pass.set_pipeline(self.sky_built.pipeline);
        pass.set_bind_group(0, self.sky_group, &[]);
        pass.draw(3, 1, 0, 0);
        self.stars.draw(&mut pass);
        // The actors, whose clips play, and the clouds the kernel lists, the
        // cumulus, which the wind carries: drawn over the world against its
        // depth, their occlusion open.
        let over = self.models.over;
        self.skinned
            .draw(&mut pass, draws.of(PIPELINE_SKINNED), b, Stage::Lit, over);
        self.decor.draw(&mut pass, &draws, b, Stage::Lit, over);
        self.impostor
            .draw(&mut pass, draws.of(PIPELINE_IMPOSTOR), b, over);
        self.clouds.draw(&mut pass);
        self.puck.draw_beacon(&mut pass, &draws);
        self.puck.draw_zone(&mut pass, &draws, b);
        self.draw_boxes(&mut pass, &draws);
        self.decor
            .draw_ghosts(&mut pass, &draws, b, self.models.frame);
        pass.end();
        let gathered = self
            .resolve
            .draw(&mut ctx, self.frame_buf, world, quiet, region);
        // At the canvas's size, as what is drawn over it: counted with the
        // markers', not the world's pixels.
        self.finish.draw(
            &mut ctx,
            GpuSpanName::marker,
            self.frame_buf,
            world,
            gathered,
            canvas_view,
        );
        self.marker
            .draw(&mut ctx, canvas_view, world.screen_depth_view, &draws, b);
        self.debug
            .draw(&mut ctx, canvas_view, world.screen_depth_view, &draws, b);
        self.draw_cases(
            &mut ctx,
            Self::cases(&draws, input.cases),
            uniforms,
            uploads_moved || tables || exposed,
        );
        let written = stamps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_written())
            .fold(0, |m, (k, _)| m | 1 << k);
        let asked = g.submit_frame(encoder, written);
        release(canvas_view);
        // The read that landed is the one that was on its way; this frame's
        // takes its place where one was asked.
        let gpu_read = q != 0 && g.has_gpu_landed();
        let (gpu_spans, gpu_px, gpu_frame) = if gpu_read {
            (
                core::array::from_fn(|k| g.gpu_ms(k as u32)),
                self.read_px,
                self.read_frame,
            )
        } else {
            ([0.0; GPU_SPAN_NAMES], 0.0, 0.0)
        };
        let frame = DRAWN_FRAMES.get(|| 0.0);
        *frame += 1.0;
        if asked {
            self.read_px = f64::from(world.width) * f64::from(world.height);
            self.read_frame = *frame;
        }
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
            self.screen.on_dropped();
            for case in self.cases.iter_mut().flatten() {
                case.read.forget();
            }
            read.forget();
            self.listed.forget();
            self.beside.forget();
            self.ao_read.forget();
        } else {
            self.resolve.on_drawn(quiet, region.is_some());
            // The jitter's sequence run through once anything moved, as
            // nothing else asks for those frames while it stands. What
            // moves on its own (an actor, the clouds, the puck) wakes the
            // turn at its own steps and takes a place of the sequence a
            // step, held to the pixels about it: it is not hurried to the
            // display's rate.
            if self.resolve.gathering() && (quiet || !animated) {
                REDRAW.set(true);
            }
            if self.screen.grows() {
                self.screen.on_drawn();
            }
        }
        read.end();
        self.frame_read = read;
        let kind = if skipped {
            FrameKind::dropped
        } else if region.is_some() {
            FrameKind::region
        } else {
            self.screen.kind
        };
        let mut stat = self.stat(draws.records, world, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
        stat.gpu_px = gpu_px;
        stat.gpu_frame = gpu_frame;
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
        ctx: &mut RenderContext,
        cases: impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])>,
        uniforms: &[f32],
        moved: bool,
    ) {
        for (id, model, image, own) in cases {
            let Some(s) = surface(id) else { continue };
            let Some(case) = self.cases[handle_slot(id) as usize].as_mut() else {
                continue;
            };
            case.draw(ctx, s, self.frame, own, uniforms, model, image, moved);
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
                PIPELINE_GHOST_BOX => d[1] * BOX_VERTICES / 3,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, idx_cnt)| idx_cnt / 3),
                PIPELINE_PUCK_ZONE => BOX_VERTICES / 3 * d[1],
                PIPELINE_PUCK_BEACON => BEACON_VERTICES / 3,
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
            gpu_frame: 0.0,
            frame: *DRAWN_FRAMES.get(|| 0.0),
            uploads: b.cnt as f64,
            upload_bytes: b.bytes as f64,
            // The depth targets: the world's own and the canvas's, one f32 a
            // pixel, AO's and the markers' regions'.
            depth_bytes: px * 4.0
                + f64::from(world.screen.0) * f64::from(world.screen.1) * 4.0
                + self.ao.depth_bytes()
                + self.marker.depth_bytes(),
            pack_bytes: self.pack.bytes as f64,
            census,
            gpu_spans,
            rays: self.screen.tally_sums(),
        }
    }
}

/// A new size is applied once it has held SETTLE_TURNS turns; until then the
/// page scales the old buffer.
fn targets(g: &Gpu, s: &mut Surface) {
    let ((width, height), screen) = (s.render_px(), s.screen_px());
    if !s.configured {
        g.configure(s.handle);
        s.configured = true;
        s.depth = 0;
    }
    let made = s.depth != 0;
    if made && s.width == width && s.height == height && s.screen == screen {
        s.waited = 0;
        return;
    }
    if made {
        if s.wanted != ((width, height), screen) {
            s.wanted = ((width, height), screen);
            s.waited = 0;
        }
        s.waited += 1;
        if s.waited < SETTLE_TURNS {
            REDRAW.set(true);
            return;
        }
    }
    s.waited = 0;
    g.resize(s.handle, screen.0, screen.1);
    for id in s.targets() {
        release(id);
    }
    let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
    s.depth = g.texture(width, height, 1, Format::depth32float, 1, usage);
    s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
    s.color = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
    s.color_view = g.view(s.color, false, 0, 1, 0, 1);
    s.screen_depth = g.texture(screen.0, screen.1, 1, Format::depth32float, 1, usage);
    s.screen_depth_view = g.view(s.screen_depth, false, 0, 1, 0, 1);
    // The world's alone holds the cast's rays and its color over frames.
    (s.rays, s.rays_view) = (0, 0);
    (s.history, s.history_view) = ([0; 2], [0; 2]);
    if s.role == surface_role::WORLD {
        for k in 0..2 {
            // Copied within where a region alone is drawn again (gpu/resolve.rs).
            let copied = usage | texture_usage::COPY_SRC | texture_usage::COPY_DST;
            s.history[k] = g.texture(width, height, 1, WORLD_COLOR, 1, copied);
            s.history_view[k] = g.view(s.history[k], false, 0, 1, 0, 1);
        }
        // Written by the cast, read by the screen pass.
        let rays = texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING;
        s.rays = g.texture(width, height, ray::WORDS, Format::r32uint, 1, rays);
        s.rays_view = g.view(s.rays, true, 0, 1, 0, ray::WORDS);
    }
    s.width = width;
    s.height = height;
    s.screen = screen;
}

/// A frame's cast handed and never drawn: the edits its decor named, held
/// for the next that is.
fn carry_edits(cast: &Option<Cast>) {
    let Some(d) = cast.as_ref().map(|c| &c.decor) else {
        return;
    };
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        r.screen.carry_edits(d.edits);
    }
}

/// Without a device or the world's canvas, the lists are recorded for the frame that has both.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == surface_role::WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), device(), world) else {
        carry_edits(&input.cast);
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), input.lists);
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: false,
            stat: None,
        };
    };
    // The surfaces would block: skip rather than wait. The uploads it
    // takes the next frame does not see, so it draws whole.
    if !(input.admit)() {
        carry_edits(&input.cast);
        take_lists(Some(g), input.lists);
        if let Some(r) = RENDERER.get(|| None).as_mut() {
            r.beside.forget();
        }
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
        redraw: *REDRAW.get(|| false) || api::images_landing(),
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
