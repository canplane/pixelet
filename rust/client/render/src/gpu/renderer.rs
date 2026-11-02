// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use pxl_content::generation::GROUPS;
use pxl_engine::light::TILE as LIGHT_TILE;
use pxl_engine::ray;
use pxl_engine::{beam, eye};
use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::api_spec::surface_role;
use crate::frame::draw::uniform_of;
use crate::frame::draw::{
    stats_slot, DrawStats, Lists, DRAW_PIPELINES, DRAW_WORDS, PIPELINE_CAMERA_GRID, PIPELINE_DEBUG,
    PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_MARKER, PIPELINE_SKINNED,
    REWRITE_WORDS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{PIPELINE_SCREEN_GATE, PIPELINE_SKY};
use crate::frame::shader::word::frame::{
    CAMERA_POS, CELL, CORNER_AO, EDGE_STRENGTH, EDGE_WIDTH, EFFECT_REACH, GIZMO, JITTER,
    LIGHT_VIEW_PROJ, LOW_SPEC, MAP, RIM_INTENSITY, SCREEN, SHADOW_MOON, SHADOW_STRENGTH,
    SHADOW_TEXEL, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::word::{
    clouds, sky, BOX_VERTICES, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS,
};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, device, frame_begin, frame_commit, gpu, on_device_lost,
    pass_desc, release, sampler_entry, texture_usage, view_entry, word_bytes, Device, Entry,
    Format, Gpu, Image, Load, RenderContext, TimestampWrites, TrackedRenderPass,
};
use crate::gpu::atlas::shelf::Atlas;
use crate::gpu::atmosphere::{Atmosphere, Upload};
use crate::gpu::cloud::CloudPass;
use crate::gpu::finish::Finish;
use crate::gpu::image_map::ImageMap;
use crate::gpu::mip::MipPass;
use crate::gpu::pack::Pack;
use crate::gpu::pass::debug::DebugPass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::location::{LocationPass, BEACON_VERTICES};
use crate::gpu::pass::mark::UploadCopies;
use crate::gpu::pass::marker::MarkerPass;
use crate::gpu::pass::model::{frame_layout, on_skins_lost, FrameGroups, Stage};
use crate::gpu::pass::preview::Preview;
use crate::gpu::pass::skinned_mesh::SkinnedPass;
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::propagate::{self, Propagate};
use crate::gpu::put::put_mat;
use crate::gpu::ray::{Cast, GroundBricks, RayCounts, RaysPass};
use crate::gpu::read::Read;
use crate::gpu::resolve::Resolve;
use crate::gpu::shadow::{self, Shadow};
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
    /// Each preview: its surface, its actor's palette, which names its
    /// draw, and the skin.
    pub previews: &'a [(u32, u32, Image)],
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
    pub atlas: &'a mut Atlas,
    pub text: TextOf,
    /// Whether the surfaces take a frame now without blocking; one admitted
    /// is drawn. Asked only once nothing else stops the frame.
    pub admit: fn() -> bool,
    /// Whether its draws are counted (`FrameStat::draws`): the profiler's
    /// alone, so a frame it does not record counts none.
    pub counted: bool,
    /// The draws among `BESIDE`'s the kernel bounds, by their handles
    /// (pxl-kernel region.rs `drawn`): each that changed marks the tiles it
    /// covers and shades where it was and where it is, as every listed draw
    /// outside `BESIDE` does (gpu/pass/mark.rs).
    pub bounded_draws: &'a [u32],
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

/// The world's draws that mark no tiles: any of theirs the kernel does not
/// bound (`FrameInput::bounded`) that moved draws the frame whole.
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

pub struct FrameOutput {
    /// What the frame turn made: decided where each preview is (no device or
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
    /// were measured over, `gpu_lattice` the camera grid's lattice its eye
    /// laid (pxl-engine `beam::Grid::lattice`), 0 where it had none, and
    /// `gpu_frame` that frame's number (`frame` as it was); else they are 0,
    /// as nothing was measured with this frame.
    pub gpu_read: bool,
    pub gpu_busy_ms: f64,
    pub gpu_px: f64,
    pub gpu_lattice: f64,
    pub gpu_frame: f64,
    /// This frame's number, from 1, a frame the renderer encoded each; 0
    /// where it stood and drew nothing.
    pub frame: f64,
    /// The upload table's uploads and their bytes, and the texture pack's:
    /// each as counted, in bytes.
    pub uploads: f64,
    pub upload_bytes: f64,
    pub pack_bytes: f64,
    /// Every draw's, counted only while `counted`: its sums are the frame's
    /// draws and triangles.
    pub draws: DrawStats,
    pub gpu_spans: [f64; GPU_SPAN_NAMES],
    /// Why the light's map cast this frame, a bit each in LIGHT_MOVED's
    /// order (gpu/ray/cast.rs `light_moved`), and the boxes decor's slots were
    /// marked by to generate again (gpu/generator.rs `marked_boxes`): 0
    /// where it cast none or marked none.
    pub light_moved: f64,
    pub decor_boxes: f64,
    /// The eye's cast's and the light's counts of an earlier frame, landed
    /// with this one, by that frame's number (gpu/ray/tally.rs `RayCounts`).
    pub rays: Option<RayCounts>,
}

/// A frame's GPU time in the spans that run threads over the world's drawn
/// pixels (span.json `per_px`, `GpuSpanName::PER_PX`): what its drawn
/// resolution can buy back.
pub fn px_ms(spans: &[f64]) -> f64 {
    GpuSpanName::PER_PX.iter().map(|&s| spans[s as usize]).sum()
}

/// A frame's GPU time in the spans that run over the camera grid's lattice,
/// its corners or its cells (span.json `per_cell`, `GpuSpanName::PER_CELL`):
/// what a coarser lattice can buy back.
pub fn cell_ms(spans: &[f64]) -> f64 {
    GpuSpanName::PER_CELL
        .iter()
        .map(|&s| spans[s as usize])
        .sum()
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

/// The renderer's pipelines while their modules are made, a few a turn,
/// before it is (`render_frame`); taken into it once all are.
static MAKING: Global<Option<Pipelines>> = Global::new();

/// How much of what the renderer makes before its first frame is made, 0 to
/// 1 (`Pipelines::share`): what the shell's progress bar shows. 0 with no
/// device, 1 once it draws.
pub fn renderer_made() -> f64 {
    if let Some(r) = RENDERER.get(|| None).as_ref() {
        return r.pipelines.share();
    }
    MAKING.get(|| None).as_ref().map_or(0.0, Pipelines::share)
}

/// Each light's step, the sun's first: the height it moves before the sky as
/// drawn moves a display level, or out of reach before it comes into reach
/// (gpu/atmosphere.rs `steps`); 0, the least, before any has landed; none
/// to take without a renderer, which draws no sky.
pub fn sky_steps() -> [f32; 2] {
    RENDERER
        .get(|| None)
        .as_ref()
        .map_or([f32::INFINITY; 2], |r| r.atmosphere.steps())
}

static REDRAW: Global<bool> = Global::new();

/// The frames encoded since boot, drawn or dropped, the last one's number
/// (`FrameStat::frame`): beside the renderer, so a lost device's next
/// renderer numbers on.
static DRAWN_FRAMES: Global<f64> = Global::new();

/// The number the frame being made takes as it is encoded, which a pass
/// that counts it names its counts by (gpu/ray/tally.rs `Frame`): a frame
/// that stands takes none, and none counts.
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
        if let Some(preview) = r.previews[slot].take() {
            preview.release();
        }
    }
}

pub struct Renderer {
    align: u32,
    /// The spans' queries (`Gpu::spans`); 0 where the device keeps no
    /// timestamps, and no frame is timed.
    query_set: u32,
    /// The world's pixels, the camera grid's lattice and the number of the
    /// frame whose spans each read on its way timed (`Gpu::submit_frame`).
    read_px: [f64; api::READS],
    read_lattice: [f64; api::READS],
    read_frame: [f64; api::READS],
    pipelines: Pipelines,
    frame_buf: u32,
    frame: Frame,
    sky_buf: u32,
    sky: [f32; SKY_FLOATS],
    sky_built: Pipeline,
    sky_group: u32,
    ghost: Pipeline,
    draws_buf: u32,
    draws_bytes: u32,
    bind_group: u32,
    models: FrameGroups,
    shadow: Shadow,
    /// Whether the shadow map is cast and drawn this frame: the setting on
    /// (`STAGE_AT::SHADOWS`) and the sun or the moon up (`prepare`).
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
    skinned_mesh: SkinnedPass,
    location: LocationPass,
    marker: MarkerPass,
    finish: Finish,
    resolve: Resolve,
    /// The light's map's tiles that changed propagated to the screen's.
    propagate: Propagate,
    debug: DebugPass,
    screen: RaysPass,
    /// The frame's whole input as the last frame drew it (stage 2): the
    /// frame's words but the jitter, the sky's and the stars' words, the
    /// air's, the surface's textures, the draws and the uploads they read,
    /// the markers' atlas, the clouds' words and the previews, and what came or
    /// was drawn again without words.
    frame_read: Read,
    /// The jitter in the frame's words as the GPU holds them (`JITTER`).
    jitter_written: [f32; 2],
    /// The lists as the last frame drew them, apart from the rest of its
    /// input: their records, uniforms and uploads (`lists_read`), and the
    /// records and uniforms of the draws the kernel does not bound, with
    /// whether an upload they read moved (`beside`, `BESIDE`).
    lists_read: Read,
    beside: Read,
    /// The sheets' words as last written (`CloudPass::words`).
    clouds_read: Read,
    /// The gate a frame not drawn whole is drawn through, over the tiles'
    /// words and uniform (`Resolve::tiles`).
    gate: Pipeline,
    /// The light's map's tiles, the light's cast's tiles' side (pxl-engine
    /// `light::TILE`), claimed the frame they changed alone, as the
    /// map gathers nothing (gpu/tiles.rs).
    light_tiles: Tiles,
    /// The uploads the draws that mark read, as the last frame drew them
    /// (gpu/pass/mark.rs).
    last_frame_uploads: UploadCopies,
    /// The frame's words as the last frame drawn drew with them, kept
    /// beside what the marks keep of its draws (`Marks::keep`), and their
    /// buffer, written where the screen's marks draw: a draw as it was is
    /// marked by them, at the jitter it was drawn at (gpu/pass/mark.rs).
    frame_was: Frame,
    frame_was_buf: u32,
    /// The eye's view-projection as `prepare` took it, which the sheets are
    /// seen through (`sheets_mark`) and the propagation reads.
    view_proj: Mat4,
    /// What the occlusion was last found from: the frame's words, its own,
    /// the surface and whether decor is generated, the world's depth the
    /// static opaque draws lay. The frame is drawn again for more than
    /// these, and the occlusion found again only where they moved.
    ao_read: Read,
    previews: [Option<Preview>; SURFACES],
    order: Vec<u32>,
}

impl Renderer {
    /// Over `pipelines`, every shader's module made (`Pipelines::make_modules`):
    /// each of its pipelines asked for here, the cast's and the ground's
    /// among them (`RaysPass::make`), so none is asked for once it draws.
    fn new(g: &Gpu, device: Device, mut pipelines: Pipelines) -> Renderer {
        let frame_buf = g.buf(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let sky_buf = g.buf(
            word_bytes(SKY_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buf);
        let atmosphere = Atmosphere::new(g, &mut pipelines, frame_buf);
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
            air,
        );
        let query_set = if device.timestamps { g.spans() } else { 0 };
        let mut r = Renderer {
            align: device.align,
            query_set,
            read_px: [0.0; api::READS],
            read_lattice: [0.0; api::READS],
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
            screen: RaysPass::new(g, &mut pipelines),
            frame_read: Read::default(),
            jitter_written: [f32::NAN; 2],
            lists_read: Read::default(),
            beside: Read::default(),
            clouds_read: Read::default(),
            gate: pipelines.build_gate(g, PIPELINE_SCREEN_GATE),
            light_tiles: Tiles::new(g, &mut pipelines, LIGHT_TILE, 1, GpuSpanName::light_mark),
            last_frame_uploads: UploadCopies::default(),
            frame_was: [0.0; FRAME_FLOATS],
            frame_was_buf: g.buf(
                word_bytes(FRAME_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
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
            previews: [const { None }; SURFACES],
            order: Vec::new(),
        };
        r.screen.make(g, &mut r.pipelines);
        r.box_groups(g);
        r
    }

    /// Whether every pipeline it asked for is made, so it may draw: the
    /// cloud field baked the first time it is (`CloudPass::bake`).
    fn made(&mut self, g: &Gpu) -> bool {
        if !self.pipelines.made() {
            return false;
        }
        self.clouds.bake(g);
        true
    }

    /// A mark's group 0 over the frame's words `frame` and `tiles`' words,
    /// uniform and marks' box (tile_mark.wgsl), for the frame it is made in.
    fn mark_group(&self, g: &Gpu, frame: u32, tiles: &Tiles) -> u32 {
        let (words, uniform) = tiles.this_frame();
        g.bind_group(
            self.skinned_mesh.mark_layout(),
            &[
                buf_entry(0, frame),
                buf_entry(30, words),
                buf_entry(31, uniform),
                buf_entry(32, tiles.marks_box()),
            ],
        )
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

    /// `eye_far` how far the eye draws, in units; none without an eye.
    fn prepare(
        &mut self,
        g: &Gpu,
        width: u32,
        height: u32,
        screen: (u32, u32),
        env: &[f32],
        eye_far: f32,
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
        self.resolve.prepare(&m, eye_world);
        let eye = [0.0; 3];
        put_ray(&mut self.sky, sky::RAY, &m);
        self.clouds.prepare(
            [eye_world[0], eye_world[1]],
            env,
            s[STAGE_AT::CLOUDS],
            eye_far * env[ENV_AIR + 1],
        );
        self.stars.prepare(&m, env, width);
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
        // Every effect's one reach (effect.wgsl `effect_share`).
        let reach = s[STAGE_AT::EFFECT_REACH];
        u[EFFECT_REACH] = reach as f32;
        self.screen_ao = s[STAGE_AT::SCREEN_AO] != 0.0;
        if self.screen_ao {
            self.ao.prepare(&proj, width, height, reach as f32);
        }
        // The eye's own place on the map, Mercator's longitude and ordinate
        // (its units over the map's radius in them), and that radius.
        let lon = world::nearest(eye_world[0], 0.0) / MERCATOR_RADIUS_U;
        u[MAP..MAP + 3].copy_from_slice(&[
            lon as f32,
            (eye_world[1] / MERCATOR_RADIUS_U) as f32,
            MERCATOR_RADIUS_U as f32,
        ]);
        // The shadow map, as wide as the effects reach, cast from the sun
        // where it still reaches the land, else from the moon where it does;
        // none where the shadows are off, or where neither reaches it, as
        // only their direct light reads the map (atmos_light.wgsl
        // `surface_light_in`) and a shadow would show nowhere.
        let (sun, moon) = (&env[ENV_SUN..ENV_SUN + 3], &env[ENV_MOON..ENV_MOON + 3]);
        let low = least_height(f64::from(env[ENV_AIR]));
        let light = if f64::from(sun[2]) > low {
            Some((sun, false))
        } else if f64::from(moon[2]) > low {
            Some((moon, true))
        } else {
            None
        };
        let light = light.filter(|_| s[STAGE_AT::SHADOWS] > 0.0 && reach > 0.0);
        self.shadows = light.is_some();
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_MOON] = if light.is_some_and(|l| l.1) { 1.0 } else { 0.0 };
        if let Some((dir, _)) = light {
            // Up to a power of two, so the snap grid and the texel move only in
            // steps; what a receiver reads within, the effects' reach.
            let half_width = libm::exp2(libm::ceil(libm::log2(reach)));
            let reach_ndc = (reach / half_width) as f32;
            self.shadow
                .aim(dir, eye, half_width, reach_ndc, eye_world.map(|v| -v));
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
            u[SHADOW_TEXEL] = self.shadow.texel as f32;
        }

        self.atmosphere.prepare(g, env, eye_far);
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
            pass.set_pipeline(self.ghost.id);
            pass.set_bind_group(0, self.bind_group, &[]);
            pass.set_vertex_buf(0, buf, 0, 0);
            pass.set_vertex_buf(1, buf, 0, 0);
            pass.draw(BOX_VERTICES, box_cnt, uniform * BOX_VERTICES, first_box);
        }
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

    /// The uploads the listed draws that mark read, each once
    /// (gpu/pass/mark.rs `UploadCopies`).
    fn marks_read(&self, draws: &Draws) -> Vec<u32> {
        let mut out = Vec::new();
        self.skinned_mesh
            .marks_read(draws.of(PIPELINE_SKINNED), &mut out);
        self.decor.marks_read(draws, &mut out);
        self.impostor
            .marks_read(draws.of(PIPELINE_IMPOSTOR), &mut out);
        self.location.marks_read(draws, &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }

    /// A turn whose lists were taken and no frame drawn: the uploads they
    /// moved the next frame does not see, so it draws whole, and what the
    /// marks read is copied again (`UploadCopies::forget`).
    fn put_off(&mut self) {
        self.frame_read.forget();
        self.beside.forget();
        self.last_frame_uploads.forget();
    }

    /// Whether a draw the kernel does not bound moved since the last
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
    /// which the wind carries, the location indicator, the ghosts, the markers, the gizmos
    /// (stage 2, `frame_read`). The rays hold the world as the cast left
    /// it, so the frame draws it again off them.
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
        read.mark(self.pack.step(g, &self.mips, input.pack));
        read.mark(self.moon.step(g, &self.mips, input.moon));
        read.mark(self.atmosphere.earth.step(g, &self.mips, input.earth));
        let mut cast = input.cast;
        // What the frame cannot put off is recorded before what can wait:
        // the cast pass made first, then the bricks' uploads, which take
        // only the room left.
        if cast.is_some() {
            self.screen
                .light_made(g, stage_lane()[STAGE_AT::SHADOWS] > 0.0);
        }
        self.screen.hand(g, &mut input.ground);
        let tiles = input.earth_tiles;
        read.mark(
            self.atmosphere
                .set_earth_tiles(g, tiles.window, tiles.layers, tiles.upload),
        );
        read.mark(
            self.marker
                .sync_atlas(g, &self.mips, input.atlas, input.text),
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
        let l = &mut self.lists_read;
        l.begin();
        l.mark(uploads_moved);
        let uniforms_moved = l.read(uniforms);
        let lists_moved = l.read(records) | uniforms_moved;
        let listed_same = l.end();
        let beside_moved = self.beside_moved(records, uniforms, &touched, input.bounded_draws);
        read.read(input.previews);

        targets(g, world);
        let animated = input.animated;
        // How far the eye draws, as far as the clouds are drawn.
        let far = cast
            .as_ref()
            .map_or(0.0, |c| f32::from_bits(c.eye[eye::FAR]));
        // The lattice the eye's grid lays (pxl-engine `beam::GRID_LATTICE`),
        // which a read of this frame was timed on.
        let lattice = cast
            .as_ref()
            .map_or(0.0, |c| f64::from(c.eye[eye::GRID + beam::GRID_LATTICE]));
        self.prepare(
            g,
            world.width,
            world.height,
            world.screen,
            input.environment,
            far,
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
        // Where the sheets the wind carries are seen, where it moved them:
        // the screen's tiles they mark, as the draws that changed mark
        // theirs drawn (`prepare_marks`).
        let clouds_moved = {
            let r = &mut self.clouds_read;
            r.begin();
            r.read(self.clouds.words());
            !r.end()
        };
        let sheets = clouds_moved
            .then(|| self.sheets_mark(input.environment, world.width, world.height))
            .flatten();
        // The sheets seen moving draw a frame however still the rest.
        let clouds_drawn = sheets.is_some();
        let ao_remade = self.screen_ao
            && self
                .ao
                .ensure(g, world.width, world.height, world.depth_only_view);
        if ao_remade {
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
        // The camera gizmo listed tints the rays' cells (screen_shade.wgsl `cell_share`).
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
        self.skinned_mesh
            .prepare(g, draws.of(PIPELINE_SKINNED), uniforms, lists_moved, b);
        self.location.prepare(g, &draws, uniforms, lists_moved, b);
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
        // Drawn through the gate where nothing but the listed draws that mark
        // moved so far: what moved marks the tiles, and those still
        // gathering since they last changed stay claimed; the cast takes
        // them, its pixels cast there alone.
        let partial = read.same() && !beside_moved;
        // The tiles of this size, before anything binds them: the claimed
        // the pixels' cast casts, its samples' raster draws and the rays'
        // screen pass draws. The depth's bound of this size with them, which
        // the world's pass sets where the shadows are cast
        // (`Propagate::set_bounds`).
        let size = [world.width, world.height];
        self.resolve.tiles.size(g, size);
        if self.shadows {
            self.propagate.size(g, size);
        }
        let drawn = self.resolve.tiles.this_frame();
        let [across, down] = self.resolve.tiles.squares();
        self.screen.set_tiles(drawn, across * down);
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
            next_frame(),
        );
        // The frame draws the decor generated, so it reads whether that is on:
        // decor turned off generates nothing and casts no ray again, yet the
        // frame stands without it.
        read.read(&[u32::from(self.screen.generated().is_some())]);
        self.prepare_previews(g, &draws, uniforms, input.previews, lists_moved);
        let q = self.query_set;
        // A query no pass wrote holds an earlier frame's value, so which were written is passed along.
        let mut stamps: [TimestampWrites; GPU_SPAN_NAMES] =
            core::array::from_fn(|span| TimestampWrites::of(q, span));
        // What outlives the frame is drawn and submitted ahead of it: from here on
        // the frame only describes itself.
        let ahead = g.encoder();
        let baked = self.impostor.bake(g, ahead, &self.decor, b);
        read.mark(baked);
        let mut ctx = RenderContext::new(g, ahead, &mut stamps);
        let gridded = self.screen.grids(&mut ctx);
        let folded = self.screen.fold(&mut ctx);
        let (tables, exposed, copied) = self.atmosphere.draw(&mut ctx);
        read.mark(tables);
        if baked || gridded || folded || tables || exposed || copied {
            g.submit(ahead);
        } else {
            release(ahead);
        }
        self.atmosphere.read_back(g);
        self.screen.read_back(g, &mut input.ground);
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
        // Only the listed draws that mark moved: the claimed tiles alone are
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
            let mut stat = self.stat(draws.records, input.counted, [0.0; GPU_SPAN_NAMES]);
            // Nothing drawn, so no number a read could name.
            stat.frame = 0.0;
            stat.rays = self.screen.take_counts();
            self.order = order;
            return (FrameKind::stood, stat);
        }

        // The listed draws that changed, where the frame is gated: each
        // marks the screen's tiles drawn where it was and where it is, and a
        // caster the map's (gpu/pass/mark.rs), a mark the host counts as
        // handed. What they read kept as the last frame drew it, made before
        // the frame's commands.
        let (screen_marking, casters_marking) = if gated {
            let k = &self.last_frame_uploads;
            let t = &touched;
            let actors =
                self.skinned_mesh
                    .prepare_marks(g, draws.of(PIPELINE_SKINNED), uniforms, t, b, k);
            let (decor, decor_casts) = self.decor.prepare_marks(g, &draws, uniforms, t, b, k);
            let impostors =
                self.impostor
                    .prepare_marks(g, draws.of(PIPELINE_IMPOSTOR), uniforms, t, b, k);
            let location = self.location.prepare_marks(g, &draws, uniforms, t, b, k);
            (
                actors || decor || impostors || location,
                actors || decor_casts,
            )
        } else {
            (false, false)
        };
        let read_uploads = self.marks_read(&draws);
        self.last_frame_uploads.prepare(g, &read_uploads, b);
        // The sun's map: whole where it moved, but where the frame is gated
        // over the world the sun's cast left as it was, through its gate,
        // the tiles the casters that changed set, or none of it. Which tiles
        // they set the GPU alone knows, so the map draws its claimed tiles
        // among those of the box they cover, which the GPU keeps and counts
        // (`Tiles::box_args`).
        // No mark is handed: through the gate the casters mark drawn, and a
        // map drawn whole claims every tile by its uniform, its stamp read
        // by no frame while the map's `claimed_for` is 1 (`Tiles::mark`).
        let cast = self.screen.kind != FrameKind::plain;
        let map_gated = gated && cast && !self.screen.light_casts();
        self.light_tiles.size(g, [shadow::SIZE; 2]);
        self.light_tiles.mark(g, (map_gated, false), None);
        let map_drawn = self.shadows && (!map_gated || casters_marking);
        // Where the map's tiles changed through its gate, the screen's tiles
        // whose pixels read them are propagated to on the GPU
        // (gpu/propagate.rs): only
        // where a caster marks, which marks the screen's tiles too
        // (`screen_marking`), so the host counts the frame as marked.
        let propagating = map_drawn && map_gated;
        if propagating {
            let spread = propagate::spread(SHADOW_MARGIN as f32);
            let light = &self.shadow.light_view_proj;
            self.propagate
                .prepare(g, &self.view_proj, light, spread, shadow::SIZE);
        }
        // The tiles marked by what moved where only the listed draws that
        // mark did, however much of the screen they and the tiles still
        // gathering cover; every one where anything else did. What the gate,
        // the resolve and the finish draw.
        self.resolve.tiles.mark(g, (gated, quiet), sheets);
        if screen_marking {
            let now = self.resolve.tiles.now();
            self.resolve.tiles.marked_in(now);
        }

        // The frame proper changes nothing that outlives it and lets go in it
        // each object it makes: one too large for the stream is dropped whole,
        // and the screen keeps the last.
        frame_begin();
        let encoder = g.encoder();
        let mut ctx = RenderContext::new(g, encoder, &mut stamps);
        // The screen's tiles' words first, then its marks, then the light's
        // words and marks, each before anything reads them; a frame dropped
        // leaves them as they were, and the next is drawn whole. A span times
        // from its first pass to its last (gpu/api.rs `TimestampWrites`), so
        // the screen's (`mark`), the light's (`light_mark`) and the
        // propagation's (`propagate`) passes are each encoded together.
        self.resolve.tiles.dispatch_marks(&mut ctx);
        if screen_marking {
            // A draw as it is marked by this frame's words, as it was by the
            // last's: each at the jitter it is or was drawn at.
            g.write_buf(self.frame_was_buf, 0, bytes(&self.frame_was));
            // The marks' groups 0 over the frame's words, this frame's and
            // the last's, and the screen tiles' (tile_mark.wgsl).
            let tiles = &self.resolve.tiles;
            let frames =
                [self.frame_buf, self.frame_was_buf].map(|frame| self.mark_group(g, frame, tiles));
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
            .load_stencil();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            self.skinned_mesh.draw_marks(&mut pass, frames, b);
            self.decor.draw_marks(&mut pass, frames, b, false);
            self.impostor.draw_marks(&mut pass, frames);
            self.location.draw_marks(&mut pass, frames);
            pass.end();
            frames.into_iter().for_each(release);
        }
        if map_drawn {
            self.light_tiles.dispatch_marks(&mut ctx);
        }
        // The same from the light, where the map is drawn through its gate:
        // the map's tiles the casters' shadows change, their words set by
        // their drawn selves, which the propagation reads.
        if propagating {
            let (light_frame, map) = self.shadow.frame_and_map();
            let group = self.mark_group(g, light_frame, &self.light_tiles);
            let desc = pass_desc(
                0,
                Load::Clear,
                map,
                Load::ReadOnly,
                1.0,
                ctx.stamp(GpuSpanName::light_mark).pass(),
            )
            .load_stencil();
            let pass = g.pass(ctx.encoder, &desc);
            let mut pass = TrackedRenderPass::new(g, pass);
            // The light's frame has no jitter, and stands in a frame whose
            // map is drawn through its gate: as a draw is and as it was, one.
            let frames = [group; 2];
            self.skinned_mesh.draw_marks(&mut pass, frames, b);
            self.decor.draw_marks(&mut pass, frames, b, true);
            pass.end();
            release(group);
            // The box they cover counted into the gate's draw.
            self.light_tiles.dispatch_box_args(&mut ctx);
        }
        if propagating {
            let depth = world.depth_only_view;
            self.propagate
                .dispatch(&mut ctx, depth, &self.light_tiles, &self.resolve.tiles);
        }
        self.screen.dispatch(&mut ctx);
        // The shadow map: the world's depth from the sun, where the rays hold
        // the world it was cast with, then decor's and the actors' drawn over
        // it; through its gate where `map_gated`, its stencil numbered as the
        // world's pass's.
        if map_drawn {
            let tiles = &mut self.light_tiles;
            let mut pass = self.shadow.begin(&mut ctx, cast, tiles, map_gated);
            let light = self.shadow.light_group;
            self.decor
                .draw_generated(&mut pass, self.screen.generated(), b, Stage::Depth, light);
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
        // The occlusion, a field over the screen as the light's map is over
        // the world: found again from the depth the static opaque draws lay
        // where what it reads moved, held where nothing did (gpu/ao.rs). A
        // gated frame moves nothing it reads: what the kernel animates is
        // drawn after.
        let ao_found = self.screen_ao && {
            let a = &mut self.ao_read;
            a.begin();
            // Not the jitter, as the light's receivers do not read it: the map
            // found at one jitter stands under the next, a field coarser
            // than the pixel the jitter moves.
            a.read(&self.frame[..JITTER]);
            a.read(self.ao.words());
            a.read(&[
                world.width,
                world.height,
                u32::from(self.screen.generated().is_some()),
            ]);
            // The world's depth is the rays', so a cast where the world they
            // read moved, or decor generated again, lays the occlusion again;
            // not one where only the jitter moved (`FrameKind::px`).
            a.mark(ao_remade || self.screen.generates() || self.screen.kind == FrameKind::cast);
            !a.end() && !gated
        };
        // A frame not drawn whole keeps the rest as the last frame left it:
        // its gate clears the claimed tiles first and writes this frame's
        // number into their stencil, and every draw after it passes there
        // alone (`Stencil`). The stencil kept but where the numbers come
        // round, cleared then; a frame drawn whole clears it, its number 0.
        let load = if gated { Load::Load } else { Load::Clear };
        let desc = pass_desc(
            world.color_view,
            load,
            world.depth_view,
            load,
            0.0,
            ctx.stamp(GpuSpanName::world).pass(),
        );
        let (desc, number) = self.resolve.tiles.gate(desc, gated);
        let pass = g.pass(encoder, &desc);
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(number) = number {
            pass.set_stencil_reference(number);
            let tiles = self.resolve.tiles.this_frame();
            let group = g.bind_group(
                self.gate.layouts[0],
                &[buf_entry(30, tiles.0), buf_entry(31, tiles.1)],
            );
            let [across, down] = self.resolve.tiles.squares();
            pass.set_pipeline(self.gate.id);
            pass.set_bind_group(0, group, &[]);
            pass.draw(6, across * down, 0, 0);
            release(group);
        }
        // The world that stands while the eye does: what the occlusion reads.
        self.draw_opaque(&mut pass, b, Stage::Lit);
        self.screen.draw(&mut pass);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor.draw_generated(
            &mut pass,
            self.screen.generated(),
            GROUPS,
            self.models.frame,
        );
        if ao_found {
            // The pass ends about the occlusion only where it is found again,
            // in a frame drawn whole: it reads the depth just laid, and the
            // rest is drawn over it, the color and depth as left, the stencil
            // 0 as the frame drawn whole cleared it.
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
            pass = TrackedRenderPass::new(g, g.pass(ctx.encoder, &desc));
        }
        // Its map over the color in the tiles drawn again, as the tiles'
        // quads, before what the kernel animates is drawn.
        if self.screen_ao {
            let [across, down] = self.resolve.tiles.squares();
            self.ao.apply(g, &mut pass, drawn, across * down);
        }
        // At the far plane after the opaque world, so only empty pixels run the sky shader.
        pass.set_pipeline(self.sky_built.id);
        pass.set_bind_group(0, self.sky_group, &[]);
        pass.draw(3, 1, 0, 0);
        self.stars.draw(&mut pass);
        // The actors, whose clips play, and the clouds the kernel lists, the
        // cumulus, which the wind carries: drawn over the world against its
        // depth, the occlusion not laid over them.
        let frame_group = self.models.frame;
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
        self.clouds.draw(&mut pass);
        self.location.draw_beacon(&mut pass, &draws, frame_group);
        self.location.draw_zone(&mut pass, &draws, b);
        self.draw_boxes(&mut pass, &draws);
        self.decor
            .draw_ghosts(&mut pass, &draws, b, self.models.frame);
        pass.end();
        let squares = self.resolve.tiles.squares();
        // What the listed draws were drawn with this frame, the frame's
        // words with them, kept for the next's marks where they were.
        self.frame_was = self.frame;
        self.skinned_mesh.keep(draws.of(PIPELINE_SKINNED), uniforms);
        self.decor.keep(&draws, uniforms);
        self.impostor.keep(draws.of(PIPELINE_IMPOSTOR), uniforms);
        self.location.keep(&draws, uniforms);
        self.last_frame_uploads
            .keep(g, ctx.encoder, &read_uploads, &touched, b);
        // The depth's bound the propagation reads, in the tiles the world's
        // pass drew, the claimed. None without the shadows, which the
        // propagation runs with alone: the frame they come on in moves the
        // frame's words (`SHADOW_STRENGTH`), so it is drawn whole and sets
        // every square before a frame reaches.
        if self.shadows {
            self.propagate
                .set_bounds(&mut ctx, world.depth_only_view, drawn, squares);
        }
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
            Self::previews(&draws, input.previews),
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
        // The read that landed is one that was on its way, of the frame it
        // timed; this frame's is kept in the read it was asked in.
        let landed = g.gpu_landed().filter(|_| q != 0);
        let gpu_read = landed.is_some();
        let (gpu_spans, gpu_busy_ms, gpu_px, gpu_lattice, gpu_frame) = match landed {
            Some(r) => (
                core::array::from_fn(|k| g.gpu_ms(k as u32)),
                g.gpu_busy_ms(),
                self.read_px[r],
                self.read_lattice[r],
                self.read_frame[r],
            ),
            None => ([0.0; GPU_SPAN_NAMES], 0.0, 0.0, 0.0, 0.0),
        };
        let frame = DRAWN_FRAMES.get(|| 0.0);
        *frame += 1.0;
        if let Some(r) = asked {
            self.read_px[r] = f64::from(world.width) * f64::from(world.height);
            self.read_lattice[r] = lattice;
            self.read_frame[r] = *frame;
        }
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
            self.screen.on_dropped();
            for preview in self.previews.iter_mut().flatten() {
                preview.read.forget();
            }
            read.forget();
            self.lists_read.forget();
            self.beside.forget();
            self.ao_read.forget();
            // The stencil holds what the last drawn frame left: the next
            // gated frame clears it.
            self.resolve.tiles.on_dropped();
            self.light_tiles.on_dropped();
            self.last_frame_uploads.forget();
        } else {
            self.resolve.on_drawn(gated);
            // The jitter's sequence run through once anything moved, as
            // nothing else asks for those frames while it stands. What
            // moves on its own (an actor, the clouds, the location indicator) wakes the
            // turn at its own steps and takes a place of the sequence a
            // step, held to the pixels about it.
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
        let mut stat = self.stat(draws.records, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
        stat.gpu_busy_ms = gpu_busy_ms;
        stat.gpu_px = gpu_px;
        stat.gpu_lattice = gpu_lattice;
        stat.gpu_frame = gpu_frame;
        stat.rays = self.screen.take_counts();
        self.order = order;
        (kind, stat)
    }

    /// The previews drawn this frame: a surface still a preview, its actor's model
    /// and its draw, found by the palette that is the preview's alone, since
    /// another actor may play the same model.
    fn previews<'a, 'd>(
        draws: &'a Draws<'d>,
        previews: &'a [(u32, u32, Image)],
    ) -> impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])> + 'a {
        previews.iter().filter_map(move |&(id, palette, image)| {
            surface(id).filter(|s| s.role == surface_role::PREVIEW)?;
            let own = draws.of(PIPELINE_SKINNED).find(|d| d[1] == palette)?;
            Some((id, own[2], image, own))
        })
    }

    /// Each preview's targets, passes and skin, made here so drawing makes nothing
    /// that outlives its frame.
    fn prepare_previews(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        previews: &[(u32, u32, Image)],
        moved: bool,
    ) {
        for (id, model, image, own) in Self::previews(draws, previews) {
            let Some(s) = surface(id) else { continue };
            targets(g, s);
            let slot = handle_slot(id) as usize;
            if self.previews[slot].is_none() {
                self.previews[slot] = Some(Preview::new(
                    g,
                    &mut self.pipelines,
                    &self.shadow,
                    self.atmosphere.entries(),
                    self.align,
                ));
            }
            let preview = self.previews[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            preview.skinned_mesh.set_skins(&[(model, image)]);
            preview
                .skinned_mesh
                .prepare(g, core::iter::once(own), uniforms, moved, uploads());
        }
    }

    /// Each preview whose picture's makings moved, `moved` what moved without
    /// words: the models' uploads, the air's tables or its exposure.
    fn draw_cases<'d>(
        &mut self,
        ctx: &mut RenderContext,
        previews: impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])>,
        uniforms: &[f32],
        moved: bool,
    ) {
        for (id, model, image, own) in previews {
            let Some(s) = surface(id) else { continue };
            let Some(preview) = self.previews[handle_slot(id) as usize].as_mut() else {
                continue;
            };
            preview.draw(ctx, s, self.frame, own, uniforms, model, image, moved);
        }
    }

    /// What the scheduler asked for: a draw passed over because its upload, skin or
    /// sprite is not on the device yet is still counted. The draws are counted
    /// only while `counted`, the profiler's, so a frame no one reads counts none.
    fn stat(
        &self,
        draws: &[[u32; DRAW_WORDS]],
        counted: bool,
        gpu_spans: [f64; GPU_SPAN_NAMES],
    ) -> FrameStat {
        let b = uploads();
        let mut stats: DrawStats = [[0.0; 2]; DRAW_PIPELINES.len()];
        for d in draws.iter().filter(|_| counted) {
            let tris = match d[4] {
                PIPELINE_GHOST_BOX => d[1] * BOX_VERTICES / 3,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, idx_cnt)| idx_cnt / 3),
                PIPELINE_LOCATION_ZONE => BOX_VERTICES / 3 * d[1],
                PIPELINE_LOCATION_BEACON => BEACON_VERTICES / 3,
                PIPELINE_MARKER => 2 * d[1],
                // Two a segment, and the faces'.
                PIPELINE_DEBUG => d[1] + d[2] / 3,
                PIPELINE_CAMERA_GRID => 1,
                _ => 0,
            };
            let c = &mut stats[stats_slot(d[4])];
            c[0] += 1.0;
            c[1] += tris as f64;
        }
        FrameStat {
            gpu_read: false,
            gpu_busy_ms: 0.0,
            gpu_px: 0.0,
            gpu_lattice: 0.0,
            gpu_frame: 0.0,
            frame: *DRAWN_FRAMES.get(|| 0.0),
            uploads: b.cnt as f64,
            upload_bytes: b.bytes as f64,
            pack_bytes: self.pack.bytes as f64,
            draws: stats,
            gpu_spans,
            light_moved: f64::from(self.screen.light_moved()),
            decor_boxes: f64::from(self.screen.decor_boxes()),
            rays: None,
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
    s.depth = g.texture(width, height, 1, api::GATED_DEPTH, 1, usage);
    s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
    s.depth_only_view = g.depth_only_view(s.depth);
    s.color = g.texture(width, height, 1, WORLD_COLOR, 1, usage);
    s.color_view = g.view(s.color, false, 0, 1, 0, 1);
    s.screen_depth = g.texture(screen.0, screen.1, 1, Format::depth32float, 1, usage);
    s.screen_depth_view = g.view(s.screen_depth, false, 0, 1, 0, 1);
    // The world's alone holds the cast's rays and its color over frames.
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
        if let Some(r) = RENDERER.get(|| None).as_mut() {
            r.put_off();
        }
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: false,
            stat: None,
        };
    };
    // The surfaces would block: skip rather than wait (`Renderer::put_off`).
    if !(input.admit)() {
        carry_edits(&input.cast);
        take_lists(Some(g), input.lists);
        if let Some(r) = RENDERER.get(|| None).as_mut() {
            r.put_off();
        }
        return FrameOutput {
            kind: FrameKind::held,
            redraw: true,
            stat: None,
        };
    }
    // The renderer is made in three steps before it draws (gpu/pipeline.rs):
    // its shaders' modules, a few a turn; then it, every pipeline asked for
    // by WebGPU's async call; then nothing until each is made. A turn before
    // draws nothing and asks for the next.
    let slot = RENDERER.get(|| None);
    if slot.is_none() {
        let making = MAKING.get(|| None);
        if making.get_or_insert_with(Pipelines::new).make_modules(g) {
            let pipelines = making.take().or_bug(Errno::ENOTRECOVERABLE);
            *slot = Some(Renderer::new(g, device, pipelines));
        }
    }
    if !slot.as_mut().is_some_and(|r| r.made(g)) {
        carry_edits(&input.cast);
        take_lists(slot.as_ref().map(|_| g), input.lists);
        if let Some(r) = slot.as_mut() {
            r.put_off();
        }
        return FrameOutput {
            kind: FrameKind::unready,
            redraw: true,
            stat: None,
        };
    }
    let r = slot.as_mut().or_bug(Errno::ENOTRECOVERABLE);
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
