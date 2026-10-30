// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec_core::eye;
use pxl_spec_core::light::TILE as LIGHT_TILE;
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
use crate::frame::shader::record::{PIPELINE_SCREEN_GATE, PIPELINE_SKY};
use crate::frame::shader::word::frame::{
    CAMERA_POS, CELL, CORNER_AO, EDGE_STRENGTH, EDGE_WIDTH, GIZMO, JITTER, LIGHT_VIEW_PROJ,
    LOW_SPEC, MAP, RIM_INTENSITY, SCREEN, SCREEN_AO, SHADOW_MOON, SHADOW_RADIUS_NDC,
    SHADOW_STRENGTH, SHADOW_TEXEL, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::{
    clouds, sky, BOX_VERTICES, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS,
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
use crate::gpu::generated::DecorFrame;
use crate::gpu::image_map::ImageMap;
use crate::gpu::mip::MipPass;
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
use crate::gpu::reach::{self, Reach};
use crate::gpu::read::Read;
use crate::gpu::resolve::Resolve;
use crate::gpu::shadow::{self, Shadow};
use crate::gpu::sheet::shelf::Sheet;
use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::star::StarPass;
use crate::gpu::surface::{
    on_surfaces_lost, surface, surfaces, Surface, SETTLE_TURNS, SURFACES, WORLD_COLOR,
};
use crate::gpu::text::TextOf;
use crate::gpu::tiles::{self, Mark, Tiles};
use crate::gpu::upload::{uploads, Draws, Uploads};
use crate::look::atmosphere::{BOTTOM_RADIUS_KM, SUN_ANGULAR_RADIUS};
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_POLE, ENV_SUN};
use crate::param::cloud;

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
    /// tiles of the world they cover and shade, and the part of the sun's
    /// map (`marks`).
    pub moved: &'a [[f32; 5]],
    /// Each moved sphere's key, its draw's own (pxl-kernel region.rs
    /// `drawn`): an actor's its joint palette's handle, which its draw names.
    pub moved_keys: &'a [u32],
    /// The draws the spheres bound, by their handles: theirs moving draws
    /// the frame whole no more (`BESIDE`).
    pub bounded: &'a [u32],
    /// Whether what the kernel animates stepped this turn: it wakes the turn
    /// at its own steps, so the resolve asks no frame after it.
    pub animated: bool,
}

/// The texels beside a caster's that it changes, in the sun's map and,
/// through a receiver's taps, on the screen: the map's filter, a slope's
/// push and the normal's offset, with room (shadow.wgsl `shadow_visibility`).
const SHADOW_MARGIN: f64 = 4.0;

/// Where a flat polygon `p`, units from the eye, falls through `m`: the clip
/// `[x0, y0, x1, y1]` its part before the eye's plane covers (cut there, as
/// Sutherland and Hodgman clip), within the screen; none where none of it
/// is before the plane or on the screen.
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

/// The least height of a light, its sine, at which it still reaches the land
/// under an eye `alt_km` up: below the horizon by the horizon's dip there and
/// its disc's radius, as the air's light ends (atmos_lut.wgsl
/// `sun_transmittance`).
fn least_height(alt_km: f64) -> f64 {
    let r = BOTTOM_RADIUS_KM;
    -libm::sin(libm::acos(r / (r + alt_km.max(0.0))) + SUN_ANGULAR_RADIUS)
}

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

/// The numbers a gated frame writes into its claimed tiles' stencil, 1 on,
/// the stencil's eight bits but 0, which a frame drawn whole leaves: one
/// coming round clears it first, so no tile holds this frame's number but
/// those it claims (`Stencil`).
const GATE_NUMBERS: u32 = 255;

/// The world's draws the spheres do not bound (`FrameInput::moved`): any of
/// theirs that moved draws the frame whole.
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
    /// The decor the GPU generates and draws over the same world (gpu/generated.rs).
    pub decor: DecorFrame<'a>,
    /// Where each actor drawn stands, units from the eye, as its draw is
    /// placed (frame/draw.rs `Lists::stands`): receivers of the shadow map as
    /// decor is (pxl-spec-core `light::ACTORS_AT`).
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
    /// read's, `gpu_busy_ms` the time they ran, those that overlap counted
    /// once, the frame's GPU time where its passes overlap (`Gpu::gpu_busy_ms`),
    /// `gpu_px` the world's pixels the frame it timed drew, what the spans
    /// were measured over, and `gpu_frame` that frame's number (`frame` as it
    /// was); else they are 0, as nothing was measured with this frame.
    pub gpu_read: bool,
    pub gpu_busy_ms: f64,
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
/// tables, the fill and the decor's generation, the thumbnails, markers and debug
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
    /// Whether the shadow map is cast and drawn this frame: the setting on
    /// (`STAGE_AT::SHADOWS`) and the sun or the moon up (`lay_out`).
    shadows: bool,
    /// Whether the debug pass drew into the screen's depth last frame, so
    /// the finish carries it whole again (gpu/finish.rs).
    debug_drew: bool,
    ao: ScreenAo,
    screen_ao: bool,
    atmosphere: Atmosphere,
    stars: StarPass,
    clouds: CloudPass,
    mips: MipPass,
    pack: Pack,
    moon: ImageMap,
    decor: DecorPass,
    impostor: ImpostorPass,
    skinned: SkinnedPass,
    puck: PuckPass,
    marker: MarkerPass,
    finish: Finish,
    resolve: Resolve,
    /// The light's map's tiles that changed reached across to the screen's.
    reach: Reach,
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
    /// records and uniforms of the draws the spheres do not bound, with
    /// whether an upload they read moved (`beside`, `BESIDE`).
    listed: Read,
    beside: Read,
    /// The sheets' words as last written (`CloudPass::words`).
    clouds_read: Read,
    /// The gate a frame not drawn whole is drawn through, and its group
    /// over the tiles' words and uniform (`Resolve::tiles`).
    gate: Built,
    gate_group: u32,
    gate_bound: (u32, u32),
    /// The number the last frame drawn gated wrote into the claimed tiles'
    /// stencil, 0 where it was drawn whole (`GATE_NUMBERS`).
    gate_number: u32,
    /// The light's map's tiles, the light's cast's tiles' side (pxl-spec-core
    /// `light::TILE`), claimed the frame they changed alone, as the
    /// map gathers nothing (gpu/tiles.rs); and the number the last map drawn
    /// through its gate wrote into its claimed tiles' stencil, as
    /// `gate_number`.
    light_tiles: Tiles,
    light_gate_number: u32,
    /// The actors' marks' group 0 over the frame's words and the screen
    /// tiles' (`SkinnedPass::draw_marks`), and what it was made over.
    mark_frame_group: u32,
    mark_frame_bound: [u32; 3],
    /// The same from the light, over the light's frame and the map's tiles.
    light_mark_group: u32,
    light_mark_bound: [u32; 3],
    /// The eye's view-projection as `lay_out` took it, which the moved
    /// spheres are seen through (`marks`).
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
        let mips = MipPass::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let pack = Pack::new(g);
        let sky_built = pipelines.build_world(g, PIPELINE_SKY);
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
            ghost: pipelines.build_world(g, PIPELINE_GHOST_BOX),
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned: SkinnedPass::new(g, &mut pipelines, device.align),
            puck: PuckPass::new(g, &mut pipelines, frame_buf, device.align),
            marker: MarkerPass::new(g, &mut pipelines, frame_buf, device.align),
            finish: Finish::new(g, &mut pipelines),
            resolve: Resolve::new(g, &mut pipelines),
            reach: Reach::new(g, &mut pipelines),
            debug: DebugPass::new(g, &mut pipelines, frame_buf, sky_buf, device.align),
            screen: RaysPass::new(g, &mut pipelines),
            frame_read: Read::default(),
            jitter_written: [f32::NAN; 2],
            listed: Read::default(),
            beside: Read::default(),
            clouds_read: Read::default(),
            gate: pipelines.build_gate(g, PIPELINE_SCREEN_GATE),
            gate_group: 0,
            gate_bound: (0, 0),
            gate_number: 0,
            light_tiles: Tiles::new(g, &mut pipelines, LIGHT_TILE, 1),
            light_gate_number: 0,
            mark_frame_group: 0,
            mark_frame_bound: [0; 3],
            light_mark_group: 0,
            light_mark_bound: [0; 3],
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
            debug_drew: false,
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
        // The shadow map, as wide as the casters reach, cast from the sun
        // where it still reaches the land, else from the moon where it does;
        // none where the shadows are off, or where neither reaches it, as
        // only their direct light reads the map (atmos_light.wgsl
        // `surface_light_in`) and a shadow would show nowhere.
        let far = s[STAGE_AT::SHADOWS];
        let (sun, moon) = (&env[ENV_SUN..ENV_SUN + 3], &env[ENV_MOON..ENV_MOON + 3]);
        let low = least_height(f64::from(env[ENV_AIR]));
        let light = if f64::from(sun[2]) > low {
            Some((sun, false))
        } else if f64::from(moon[2]) > low {
            Some((moon, true))
        } else {
            None
        };
        let light = light.filter(|_| far > 0.0);
        self.shadows = light.is_some();
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_MOON] = if light.is_some_and(|l| l.1) { 1.0 } else { 0.0 };
        if let Some((dir, _)) = light {
            // Up to a power of two, so the snap grid and the texel move only in steps;
            // the shadows fade by the casters' own reach.
            let half_width = libm::exp2(libm::ceil(libm::log2(far)));
            let shadow_radius_ndc = (far / half_width) as f32;
            self.shadow.aim(
                dir,
                eye,
                half_width,
                shadow_radius_ndc,
                eye_world.map(|v| -v),
            );
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
            u[SHADOW_TEXEL] = self.shadow.texel as f32;
            u[SHADOW_RADIUS_NDC] = shadow_radius_ndc;
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

    /// Where the `moved` spheres fall, each apart, as this frame's eye and
    /// light see them: on the screen, the drawn pixels of a world `width` by
    /// `height` each covers but an actor's, `actors` their `keys`, which mark
    /// their own (gpu/pass/skinned.rs), which mark the screen's tiles
    /// (`Tiles::mark`);
    /// and the texels of the light's map each caster covers, all of it that
    /// moves while only they do, which mark the map's tiles, and through
    /// them, reached on the GPU, the screen's that read them (gpu/reach.rs),
    /// where a caster's shadow falls: an actor's apart, which marks no tile's
    /// word, its drawn self marking them, the map drawn in those of its
    /// tiles they claim (`Tiles::beside`). None at all where nothing moved or a
    /// sphere stands about or behind the eye.
    fn marks(
        &mut self,
        moved: &[[f32; 5]],
        keys: &[u32],
        actors: &[u32],
        width: u32,
        height: u32,
    ) -> Option<(Vec<Mark>, Vec<Mark>, Vec<Mark>)> {
        if moved.is_empty() {
            return None;
        }
        let light = self.shadow.cast_from.filter(|_| self.shadows);
        let casts = |s: &[f32; 5]| s[4] != 0.0 && light.is_some();
        let sphere = |s: &[f32; 5]| ([s[0], s[1], s[2]].map(f64::from), f64::from(s[3]));
        let in_map = |actor: bool| -> Vec<Mark> {
            moved
                .iter()
                .zip(keys)
                .filter(|&(s, k)| casts(s) && actors.contains(k) == actor)
                .filter_map(|(s, _)| {
                    let clip = clip_of(&self.shadow.light_view_proj, core::iter::once(sphere(s)))?;
                    let n = shadow::SIZE;
                    tiles::mark_of(clip, n, n, SHADOW_MARGIN as f32)
                })
                .collect()
        };
        let (in_map, actors_in_map) = (in_map(false), in_map(true));
        // Bounded only where every sphere is before the eye.
        clip_of(&self.view_proj, moved.iter().map(sphere))?;
        let marks: Vec<Mark> = moved
            .iter()
            .zip(keys)
            .filter(|(_, k)| !actors.contains(k))
            .filter_map(|(s, _)| {
                let clip = clip_of(&self.view_proj, core::iter::once(sphere(s)))?;
                tiles::mark_of(clip, width, height, tiles::MARK_MARGIN)
            })
            .collect();
        Some((marks, in_map, actors_in_map))
    }

    /// Where the cloud sheets are seen, as cloud.wgsl `vs` lays them: a
    /// square each at its height over the eye, as wide as the eye draws
    /// clouds, cut at the eye's plane: the drawn pixels they cover, which
    /// mark the screen's tiles; none where none is seen.
    fn sheets_mark(&self, env: &[f32], width: u32, height: u32) -> Option<Mark> {
        let w = self.clouds.words();
        if w[clouds::SHOWN] <= 0.0 {
            return None;
        }
        let (alt, km_per_u) = (f64::from(env[ENV_AIR]), f64::from(env[ENV_AIR + 1]));
        let reach = f64::from(w[clouds::REACH]);
        let mut clip: Option<[f32; 4]> = None;
        for km in [cloud::altocumulus::KM, cloud::cirrus::KM] {
            let h = f64::from(km) - alt;
            let half =
                libm::sqrt((reach * reach - h * h).max(0.0)) / km_per_u.max(f64::MIN_POSITIVE);
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
            if let Some(b) = plane_clip(&self.view_proj, &quad) {
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
        clip.and_then(|c| tiles::mark_of(c, width, height, tiles::MARK_MARGIN))
    }

    /// Whether a draw the spheres do not bound moved since the last
    /// frame (`BESIDE`): its record or uniform, or an upload it reads
    /// (`touched`, the handles this frame's lists upload, rewrite or free).
    fn beside_moved(
        &mut self,
        records: &[u32],
        uniforms: &[f32],
        touched: &[u32],
        bounded: &[u32],
    ) -> bool {
        let r = &mut self.beside;
        r.begin();
        for d in records.as_chunks::<DRAW_WORDS>().0 {
            if BESIDE.contains(&d[4]) && !bounded.contains(&d[0]) {
                r.read(d);
                r.read(uniform_of(uniforms, d));
                r.mark(touched.contains(&d[0]));
            }
        }
        !r.end()
    }

    /// The world image's opaque models: the decor generated over the world. The
    /// impostors are the lit pass's alone.
    fn draw_opaque(&mut self, pass: &mut TrackedRenderPass, b: &mut Uploads, stage: Stage) {
        self.decor
            .draw_generated(pass, self.screen.generated(), b, stage, self.models.frame);
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
            self.screen
                .light_made(g, stage_lane()[STAGE_AT::SHADOWS] > 0.0);
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
        let beside_moved = self.beside_moved(records, uniforms, &touched, input.bounded);
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
                self.shadow.light_words
            } else {
                [0; eye::LIGHT_WORDS]
            };
            c.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&sun);
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
        // sun's map, laid out as this frame's eye and light see it; and
        // where the sheets the wind carries are seen, where it moved them.
        let clouds_moved = {
            let r = &mut self.clouds_read;
            r.begin();
            r.read(self.clouds.words());
            !r.end()
        };
        let sheets =
            clouds_moved.then(|| self.sheets_mark(input.environment, world.width, world.height));
        // The actors mark their own tiles, drawn where they were and are
        // (gpu/pass/skinned.rs `draw_marks`): their spheres mark none.
        let actors: Vec<u32> = records
            .as_chunks::<DRAW_WORDS>()
            .0
            .iter()
            .filter(|d| d[4] == PIPELINE_SKINNED)
            .map(|d| d[1])
            .chain(self.skinned.drawn_keys())
            .collect();
        let bound = self.marks(
            input.moved,
            input.moved_keys,
            &actors,
            world.width,
            world.height,
        );
        let unbounded = !input.moved.is_empty() && bound.is_none();
        // What marks the tiles, and the light's map's part.
        let (mut marks, light_marks, actors_light_marks) = bound.unwrap_or_default();
        marks.extend(sheets.flatten());
        // The sheets seen moving draw a frame however still the rest.
        let clouds_drawn = sheets.flatten().is_some();
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
        // Drawn through the gate where nothing but what the kernel bounds
        // moved so far: what moved marks the tiles (`Resolve::age`), and
        // those still gathering since they last changed stay claimed; the
        // cast takes them, its pixels cast there alone.
        let partial = read.same() && !beside_moved && !unbounded;
        let marks: &[Mark] = if partial { &marks } else { &[] };
        // The tiles of this size, before anything binds them: the claimed
        // the pixels' cast casts and its samples' raster draws, and those the
        // rays' screen pass draws, every one where the world's pass has no
        // gate.
        self.resolve.tiles.size(g, [world.width, world.height]);
        let drawn = if g.gates() {
            self.resolve.tiles.bound()
        } else {
            self.resolve.tiles.every()
        };
        let [across, down] = self.resolve.tiles.squares();
        self.screen
            .set_tiles(self.resolve.tiles.bound(), drawn, across * down);
        self.screen.prepare(
            g,
            cast,
            &meshes,
            (world.rays_view, [world.width, world.height]),
            (
                self.shadow.world,
                self.shadow.generation,
                self.shadow.starts(),
            ),
            partial,
        );
        // The frame draws the decor generated, so it reads whether that is on:
        // decor turned off generates nothing and casts no ray again, yet the
        // frame stands without it.
        read.read(&[u32::from(self.screen.generated().is_some())]);
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
        self.reach.back(g, &mut self.resolve.tiles);
        self.screen.tally_back(g);
        if clouds_moved {
            self.clouds.write(g);
        }
        // The world as it stood: all the frame draws from read as last time
        // but its lists and its jitter, over rays cast again at most at
        // their pixels.
        let stands = read.end()
            && self.screen.kind != FrameKind::cast
            && !self.screen.generates()
            && !gridded;
        // Nothing moved but the jitter.
        let quiet = stands && listed_same && !clouds_drawn;
        // Only what the kernel bounds moved: the claimed tiles alone are
        // drawn again. Else the frame is drawn whole, its pixels cast whole.
        let gated = partial && stands;
        if !gated {
            self.screen.whole();
        }
        // The frame stands where nothing moved, the jitter neither: nothing
        // is drawn, nothing submitted, and the canvases keep the frame they
        // show.
        if quiet && !self.screen.casts_px() && !self.screen.light_casts() {
            self.frame_read = read;
            let mut stat = self.stat(draws.records, world, input.counted, [0.0; GPU_SPAN_NAMES]);
            // Nothing drawn, so no number a read could name.
            stat.frame = 0.0;
            self.order = order;
            return (FrameKind::stood, stat);
        }

        // The sun's map: whole where it moved, but where the frame is gated
        // over the world the sun's cast left as it was, through its gate,
        // the tiles the casters that moved cover, or none of it; whole where
        // the device has no stencil.
        let cast = self.screen.kind != FrameKind::plain;
        let map_gated = gated && cast && !self.screen.light_casts() && g.gates();
        let none: &[Mark] = &[];
        let (light_marks, actors_light_marks) = if map_gated {
            (&light_marks[..], &actors_light_marks[..])
        } else {
            (none, none)
        };
        self.light_tiles.size(g, [shadow::SIZE; 2]);
        self.light_tiles.mark(
            g,
            [shadow::SIZE; 2],
            (map_gated, false),
            light_marks,
            actors_light_marks,
        );
        let map_drawn = self.shadows
            && (!map_gated || !light_marks.is_empty() || !actors_light_marks.is_empty());
        // Where the map's tiles changed through its gate, the screen's tiles
        // whose pixels read them are reached on the GPU (gpu/reach.rs), what
        // it marked read back.
        let reaching = map_drawn && map_gated;
        if reaching {
            let spread = reach::spread(SHADOW_MARGIN as f32);
            let light = &self.shadow.light_view_proj;
            let marks_box = self.light_tiles.marks_box();
            self.reach
                .lay_out(g, &self.view_proj, light, spread, shadow::SIZE, marks_box);
        }
        // The tiles marked by what moved where only what the kernel bounds
        // did, however much of the screen they and the tiles still gathering
        // cover; every one where anything else did. What the gate, the
        // resolve and the finish draw.
        self.resolve
            .tiles
            .mark(g, [world.width, world.height], (gated, quiet), marks, &[]);
        // The actors whose draws changed, marking where they were and are on
        // the GPU, a mark the host counts as handed.
        let actors_marking = gated
            && self.skinned.prepare_marks(
                g,
                draws.of(PIPELINE_SKINNED),
                uniforms,
                &touched,
                uploads(),
            );
        if actors_marking {
            let now = self.resolve.tiles.now();
            self.resolve.tiles.marked_in(now);
        }

        // The frame proper changes nothing that outlives it, so one too large
        // for the stream is dropped whole and the screen keeps the last.
        frame_begin();
        let encoder = g.encoder();
        let mut ctx = RenderContext::new(g, encoder, &mut stamps);
        // The tiles' words first, before anything reads them; a frame
        // dropped leaves them as they were, and the next is drawn whole.
        self.resolve.tiles.dispatch_marks(&mut ctx);
        if map_drawn {
            self.light_tiles.dispatch_marks(&mut ctx);
        }
        if actors_marking {
            let tiles = self.resolve.tiles.bound();
            let bound = [self.frame_buf, tiles.0, tiles.1];
            if bound != self.mark_frame_bound {
                release(self.mark_frame_group);
                self.mark_frame_group = g.bind_group(
                    self.skinned.mark_layout(),
                    &[
                        buf_entry(0, self.frame_buf),
                        buf_entry(30, tiles.0),
                        buf_entry(31, tiles.1),
                    ],
                );
                self.mark_frame_bound = bound;
            }
            // The world's depth attached read only, its stencil kept: the
            // marks test nothing and write no color.
            let desc = pass_desc(
                0,
                Load::Clear,
                world.depth_view,
                Load::ReadOnly,
                0.0,
                ctx.stamp(GpuSpanName::mark).pass(),
            )
            .kept();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            self.skinned
                .draw_marks(&mut pass, self.mark_frame_group, uploads());
            pass.end();
        }
        // The same from the light, where the map is drawn through its gate:
        // the map's tiles the actors' shadows change, their words set by
        // their drawn selves, which the reach reads.
        if actors_marking && map_drawn && map_gated {
            let (light_frame, map) = self.shadow.frame_and_map();
            let tiles = self.light_tiles.bound();
            let bound = [light_frame, tiles.0, tiles.1];
            if bound != self.light_mark_bound {
                release(self.light_mark_group);
                self.light_mark_group = g.bind_group(
                    self.skinned.mark_layout(),
                    &[
                        buf_entry(0, light_frame),
                        buf_entry(30, tiles.0),
                        buf_entry(31, tiles.1),
                    ],
                );
                self.light_mark_bound = bound;
            }
            let desc = pass_desc(
                0,
                Load::Clear,
                map,
                Load::ReadOnly,
                1.0,
                ctx.stamp(GpuSpanName::mark).pass(),
            )
            .kept();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            self.skinned
                .draw_marks(&mut pass, self.light_mark_group, uploads());
            pass.end();
        }
        if reaching {
            let depth = world.depth_only_view;
            self.reach
                .dispatch(&mut ctx, depth, &self.light_tiles, &self.resolve.tiles);
        }
        self.screen.dispatch(&mut ctx);
        // The shadow map: the world's depth from the sun, where the rays hold
        // the world it was cast with, then decor's and the actors' drawn over
        // it; through its gate where `map_gated`, its stencil numbered as the
        // world's pass's.
        if map_drawn {
            let number = map_gated.then(|| self.light_gate_number % GATE_NUMBERS + 1);
            self.light_gate_number = number.unwrap_or(0);
            let mut pass = self.shadow.begin(&mut ctx, cast, &self.light_tiles, number);
            let light = self.shadow.light_group;
            self.decor
                .draw_generated(&mut pass, self.screen.generated(), b, Stage::Depth, light);
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
            u32::from(self.screen.generated().is_some()),
        ]);
        // The world's depth is the rays', so a cast that laid them again
        // lays the occlusion again.
        a.mark(ao_remade || self.screen.generates() || self.screen.casts_px());
        let ao_moved = !a.end();
        // A gated frame moves no depth the occlusion reads: its prepass
        // draws the rays and the generated decor, never what the kernel
        // animates (gpu/pass/model.rs, drawn over it), so it stands.
        if self.screen_ao && ao_moved && !gated {
            let mut pass = self.ao.begin_prepass(&mut ctx);
            self.screen.draw_depth(&mut pass);
            self.draw_opaque(&mut pass, b, Stage::Prepass);
            pass.end();
            self.ao.run(&mut ctx);
        }
        let canvas_view = g.cur(world.handle);
        // A frame not drawn whole keeps the rest as the last frame left it:
        // its gate clears the claimed tiles first and writes this frame's
        // number into their stencil, and every draw after it passes there
        // alone (`Stencil`). The stencil kept but where the numbers come
        // round, cleared then; a frame drawn whole clears it, its number 0.
        // Without the stencil the gate passes every pixel, so the world's pass
        // is drawn whole, the rest by tile all the same.
        let gating = gated && g.gates();
        let load = if gating { Load::Load } else { Load::Clear };
        let desc = pass_desc(
            world.color_view,
            load,
            world.depth_view,
            load,
            0.0,
            ctx.stamp(GpuSpanName::world).pass(),
        );
        let number = if gating {
            self.gate_number % GATE_NUMBERS + 1
        } else {
            0
        };
        let desc = if number > 1 { desc.kept() } else { desc };
        self.gate_number = number;
        let pass = g.pass(encoder, &desc);
        let mut pass = TrackedRenderPass::new(g, pass);
        if gating {
            pass.set_stencil_reference(number);
            let tiles = self.resolve.tiles.bound();
            if tiles != self.gate_bound {
                release(self.gate_group);
                self.gate_group = g.bind_group(
                    self.gate.layouts[0],
                    &[buf_entry(30, tiles.0), buf_entry(31, tiles.1)],
                );
                self.gate_bound = tiles;
            }
            let [across, down] = self.resolve.tiles.squares();
            pass.set_pipeline(self.gate.pipeline);
            pass.set_bind_group(0, self.gate_group, &[]);
            pass.draw(6, across * down, 0, 0);
        }
        self.draw_opaque(&mut pass, b, Stage::Lit);
        self.screen.draw(&mut pass);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor.draw_generated(
            &mut pass,
            self.screen.generated(),
            GROUPS,
            self.models.frame,
        );
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
        self.puck.draw_beacon(&mut pass, &draws, over);
        self.puck.draw_zone(&mut pass, &draws, b);
        self.draw_boxes(&mut pass, &draws);
        self.decor
            .draw_ghosts(&mut pass, &draws, b, self.models.frame);
        pass.end();
        // The depth's bound the reach reads, in the tiles the world's pass
        // drew: the claimed, or every one without the stencil.
        let drawn = if g.gates() {
            self.resolve.tiles.bound()
        } else {
            self.resolve.tiles.every()
        };
        let size = [world.width, world.height];
        let squares = self.resolve.tiles.squares();
        // What the actors were drawn with this frame, kept for the next's
        // marks where they were.
        self.skinned.keep(
            g,
            ctx.encoder,
            draws.of(PIPELINE_SKINNED),
            uniforms,
            uploads(),
        );
        self.reach
            .set_bounds(&mut ctx, world.depth_only_view, size, drawn, squares);
        let gathered = self.resolve.draw(&mut ctx, self.frame_buf, world, gated);
        // At the canvas's size, its own span. A frame not drawn whole
        // finishes again only its claimed tiles (screen_tiles.wgsl); not
        // after the debug pass drew into the screen's depth.
        self.finish.draw(
            &mut ctx,
            GpuSpanName::finish,
            self.frame_buf,
            world,
            gathered,
            canvas_view,
            (gated && !self.debug_drew).then_some(&self.resolve.tiles),
        );
        self.marker
            .draw(&mut ctx, canvas_view, world.screen_depth_view, &draws, b);
        self.debug_drew =
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
        let (gpu_spans, gpu_busy_ms, gpu_px, gpu_frame) = if gpu_read {
            (
                core::array::from_fn(|k| g.gpu_ms(k as u32)),
                g.gpu_busy_ms(),
                self.read_px,
                self.read_frame,
            )
        } else {
            ([0.0; GPU_SPAN_NAMES], 0.0, 0.0, 0.0)
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
            // The stencil holds what the last drawn frame left: the next
            // gated frame clears it.
            self.gate_number = GATE_NUMBERS;
            self.light_gate_number = GATE_NUMBERS;
            self.reach.on_dropped(self.resolve.tiles.now());
        } else {
            self.resolve.on_drawn(gated);
            // The jitter's sequence run through once anything moved, as
            // nothing else asks for those frames while it stands. What
            // moves on its own (an actor, the clouds, the puck) wakes the
            // turn at its own steps and takes a place of the sequence a
            // step, held to the pixels about it: it is not hurried to the
            // display's rate.
            if self.resolve.gathering() && (quiet || !animated) {
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
        } else if gated {
            FrameKind::gated
        } else {
            self.screen.kind
        };
        let mut stat = self.stat(draws.records, world, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
        stat.gpu_busy_ms = gpu_busy_ms;
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
            gpu_busy_ms: 0.0,
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
    s.depth = g.texture(width, height, 1, g.gated_depth(), 1, usage);
    s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
    s.depth_only_view = g.depth_only_view(s.depth);
    s.color = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
    s.color_view = g.view(s.color, false, 0, 1, 0, 1);
    s.screen_depth = g.texture(screen.0, screen.1, 1, Format::depth32float, 1, usage);
    s.screen_depth_view = g.view(s.screen_depth, false, 0, 1, 0, 1);
    // The world's alone holds the cast's rays and its color over frames.
    (s.rays, s.rays_view) = (0, 0);
    (s.history, s.history_view) = ([0; 2], [0; 2]);
    (s.shown, s.shown_view) = (0, 0);
    if s.role == surface_role::WORLD {
        s.shown = g.texture(screen.0, screen.1, 1, g.canvas(), 1, usage);
        s.shown_view = g.view(s.shown, false, 0, 1, 0, 1);
        for k in 0..2 {
            s.history[k] = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
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
