// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_guest::mem::base::off_of;

use crate::api_spec::surface_role;
use crate::frame::draw::{
    census_slot, uniform_of, DrawCensus, Lists, DRAW_PIPELINES, DRAW_WORDS, PIPELINE_BADGE, PIPELINE_CAMERA_GRID, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{compute, dynamic, same, within};
use crate::frame::shader::record::{
    PIPELINE_DEBUG_FACE, PIPELINE_SCREEN_PREPASS, PIPELINE_SCREEN_RAYS, PIPELINE_SKY, SHADER_BEAM, SHADER_BOUND, SHADER_CAST, SHADER_CELLS, SHADER_FORGET, SHADER_HEIGHTS, SHADER_JOINED, SHADER_MISS,
    SHADER_SAMPLE, SHADER_SUN, SHADER_SUN_MARK, SHADER_WAYS, SHADER_WAYS_FREE, SHADER_WAYS_FREED, SHADER_WAYS_TREE,
};
use crate::frame::shader::words::frame::{
    CAMERA_POS, CELL, CORNER_AO, EDGE_STRENGTH, EDGE_WIDTH, GIZMO, LIGHT_VIEW_PROJ, LOW_SPEC, MAP, RIM_INTENSITY, SCREEN_AO, SHADOW_RIM, SHADOW_STRENGTH, SHADOW_TEXEL, VIEWPORT, VIEW_PROJ, VIGNETTE,
};
use crate::frame::shader::words::{sky, DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api::{
    buffer_entry, buffer_range_entry, buffer_usage, bytes, device, device_lost, frame_begin, frame_commit, gpu, pass_desc, release, room, sampler_entry, texture_usage, view_entry, word_bytes,
    write_buffer_bytes, Device, Entry, Format, Gpu, Image, Load, Stamp, DONE, UNIFORM_ALIGN_MAX,
};
use crate::gpu::atmosphere::{Atmosphere, Upload, AIR_ENTRIES};
use crate::gpu::blocks::{blocks, Blocks, Draws};
use crate::gpu::clouds::CloudPass;
use crate::gpu::grown::{DecorFrame, Drawn, Grown, Grows};
use crate::gpu::image_map::ImageMap;
use crate::gpu::layers::{Layers, Mips};
use crate::gpu::pass::badges::BadgePass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::model::{frame_layout, skins_lost, DrawUniforms, FrameGroups, Stage};
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::read::Read;
use crate::gpu::shadow::{self, Shadow};
use crate::gpu::sheet::shelf::Sheet;
use crate::gpu::stars::StarPass;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::look::environment::{ENV_AIR, ENV_STARS, ENV_SUN};
use crate::mesh::paint::ground::look_words;
use pxl_spec_core::beam;
use pxl_spec_core::eye;
use pxl_spec_core::observed;
use pxl_spec_core::ray;
use pxl_spec_core::scatter::GROUPS;
use pxl_spec_core::sun;
use pxl_spec_core::ways;
use pxl_spec_core::window::{self, CellsBack, FreshSquare, BACK_WORDS, COLUMNS_AT, ELE_AT, HANDED, HANDED_WORDS, LEVELS, LINES_AT, LOOKS_AT, SQUARE, SQUARES, SQUARE_CELLS, SQUARE_WORDS};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const BOX_VERTICES: u32 = 36;
const DRAWS_INITIAL: u32 = 256 * DRAW_FLOATS as u32;

const _: () = assert!(GPU_SPAN_NAMES <= 32, "a span a bit of the mask the host is told");
const SPAN_AIR: usize = GpuSpanName::air as usize;
const SPAN_PREPASS: usize = GpuSpanName::prepass as usize;
const SPAN_AO: usize = GpuSpanName::ao as usize;
const SPAN_WORLD: usize = GpuSpanName::world as usize;
const SPAN_BADGES: usize = GpuSpanName::badges as usize;
const SPAN_DEBUG: usize = GpuSpanName::debug as usize;
const SPAN_CASES: usize = GpuSpanName::cases as usize;
const SPAN_FILL: usize = GpuSpanName::fill as usize;
const SPAN_CAST: usize = GpuSpanName::cast as usize;
const SPAN_GROW: usize = GpuSpanName::grow as usize;
const SPAN_SHADOW: usize = GpuSpanName::shadow as usize;

type Frame = [f32; FRAME_FLOATS];

pub struct FrameInput<'a> {
    pub lists: Lists<'a>,
    pub environment: &'a [f32],
    /// 0 while it loads.
    pub decor: &'a [(u32, Option<Image>, Extent)],
    pub skins: &'a [(u32, Image)],
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
    pub billboards: &'a mut Sheet,
    pub text: TextOf,
    /// Whether the surfaces take a frame now without blocking; one admitted
    /// is drawn. Asked only once nothing else stops the frame.
    pub admit: fn() -> bool,
    /// Whether its draws are counted (`FrameStat::census`): the profiler's
    /// alone, so a frame it does not record counts none.
    pub counted: bool,
}

/// The buffers squares' cells come back through: what a frame hands, a
/// mapping still out a frame more (pxl-spec-core `window::HANDED`).
const READS: usize = window::READS as usize;
/// A buffer of them carrying no square.
const FREE: u32 = u32::MAX;

/// The earth layer's tiles as the GPU reads them: the floor's window, its
/// south-west tile first, and per tile its layer plus one; `upload` hands each
/// tile that has come to the writer while the writer takes it.
pub struct EarthTiles<'a> {
    pub window: [i32; 2],
    pub layers: &'a [u32],
    pub upload: &'a mut Upload,
}

/// The ground's windows as the GPU holds them (pxl-spec-core `window`,
/// `SQUARES` of them): the squares to hand it, and where their cells come
/// back.
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
    /// The ways' pool's blocks at first and at most (pxl-kernel param
    /// `WAYS_POOL_BYTES`): it starts at the first and doubles each time a
    /// build runs out, never past the most, nor `ways::tree::POOL_BLOCKS`
    /// (`GroundBuffers::grow`).
    pub pool: [u32; 2],
    /// The blocks each handed square's scratch holds a tree of, at first and
    /// at most (pxl-kernel param `WAYS_SCRATCH_BYTES`): it starts at the
    /// first and doubles each time a build outgrows it, never past the most,
    /// nor `ways::tree::SQUARE_BLOCKS` (`GroundBuffers::grow_scratch`).
    pub scratch: [u32; 2],
    /// A read's records' room at first, in bytes: every read's is doubled
    /// past a square's records that outgrew it, up to
    /// `ways::tree::RECORDS_BYTES` (`GroundBuffers::record_bytes`).
    pub records: u32,
}

/// How a square's fill landed: its cells are back in every case.
pub enum Landing<'a> {
    /// Its ways' tree built: the blocks it holds, their count first, a
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
pub const RECORD_ROOMS_BYTES: u64 = window::READS as u64 * (ways::tree::RECORDS_BYTES as u64).next_power_of_two();

/// A square whose cells came back, the fill they are of
/// (`CellsBack::ticket`), and how its ways' tree landed.
pub type CellsLanded = dyn FnMut(u32, u32, Landing<'_>);

/// The GPU's cast this frame (frame/shader/cast.gen.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-spec-core `eye`; pxl-kernel view/lens.rs `gpu_cast`,
    /// scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// Per window of the eye's, from the top's, a row a slot: the square it
    /// holds, X and Y, and whether it is present, and a spare.
    pub window: &'a [i32],
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row (pxl-kernel scene/cast.rs `shards`); read where it lies,
    /// unwritten until the page replays the turn.
    pub shards: &'a [[i32; 4]],
    pub overwrites: Overwrites<'a>,
    /// The decor the GPU grows and draws over the same world (gpu/grown.rs).
    pub decor: DecorFrame<'a>,
}

/// The overwrites' tree (pxl-kernel layer/object/store.rs) as it lives: its
/// heap and data lanes, unwritten until the page replays the turn, the room
/// either has before it grows, the revision they are at, and the blocks
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
    /// The block table's blocks and their bytes, the depth targets' bytes
    /// and the texture layers': each as counted, in bytes.
    pub blocks: f64,
    pub block_bytes: f64,
    pub depth_bytes: f64,
    pub layers_bytes: f64,
    /// Every draw's, counted only while `counted`: its sums are the frame's
    /// draws and triangles.
    pub census: DrawCensus,
    pub gpu_spans: [f64; GPU_SPAN_NAMES],
}

/// A frame's GPU time: its spans' sum, the passes timed. The span from the
/// first to the last would count what lies between submits.
pub fn gpu_ms(spans: &[f64]) -> f64 {
    spans.iter().sum()
}

/// The passes drawn at the world's own pixels, whose cost goes with them:
/// its depth prepass, its occlusion, its drawing and its cast. The air's
/// tables, the fill and the decor's growth, the thumbnails, badges and debug
/// cost what they cost at any resolution.
const PER_PIXEL: [GpuSpanName; 4] = [GpuSpanName::prepass, GpuSpanName::ao, GpuSpanName::world, GpuSpanName::cast];

/// A frame's GPU time in the passes drawn at the world's pixels (`PER_PIXEL`):
/// what its drawn resolution can buy back.
pub fn pixels_ms(spans: &[f64]) -> f64 {
    PER_PIXEL.iter().map(|&s| spans[s as usize]).sum()
}

/// The ray under NDC (x, y) is `x·col0 + y·col1 + col2`, taken in f64. Un-
/// projecting two points in the shader kept a direction to only ~1e-3 rad,
/// which the secant carried out to kilometers at the horizon: the clouds shook.
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
    for (k, (to, from)) in [(point(1.0, 0.0), c), (point(0.0, 1.0), c), (c, *eye)].into_iter().enumerate() {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3].iter_mut().zip(v3::sub(&to, &from)) {
            *o = e as f32;
        }
    }
}

static RENDERER: Global<Option<Renderer>> = Global::new();

/// Each light's step, the sun's first: the height it moves before the sky as
/// drawn moves a display level (gpu/atmosphere.rs `steps`); none before any
/// view's slope has landed.
pub fn sky_steps() -> [Option<f32>; 2] {
    RENDERER.get(|| None).as_ref().map_or([None; 2], |r| r.atmosphere.steps())
}

static REDRAW: Global<bool> = Global::new();

pub fn gpu_lost() {
    RENDERER.set(None);
    skins_lost();
    device_lost();
    blocks().lose();
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

/// The dev gizmos, over the finished world in passes of their own: their
/// faces into the world's depth, cleared once the world is done with it, then
/// their lines against those faces alone, so a gizmo hides gizmos and nothing
/// of the world hides one. The camera grid is drawn over all of it, in the
/// lines' pass, off the frame's cell and the sky's ray.
///
/// Each set takes its own translation, a draw's uniform a set.
struct DebugPass {
    built: Built,
    faces: Built,
    frame_group: u32,
    uniforms: DrawUniforms,
    grid: Built,
    grid_group: u32,
}

impl DebugPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, sky_buffer: u32, align: u32) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, g.canvas());
        let faces = pipelines.build(g, PIPELINE_DEBUG_FACE, g.canvas());
        let frame_group = g.bind_group(built.layouts[0], &[buffer_entry(0, frame_buffer)]);
        let grid = pipelines.build(g, PIPELINE_CAMERA_GRID, g.canvas());
        let grid_group = g.bind_group(grid.layouts[0], &[buffer_entry(0, frame_buffer), buffer_entry(19, sky_buffer)]);
        DebugPass {
            built,
            faces,
            frame_group,
            uniforms: DrawUniforms::new(16, align),
            grid,
            grid_group,
        }
    }

    /// Each set's uniform, its translation, written where the lists it is
    /// made from `moved`.
    fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool) {
        let sets = draws.of(PIPELINE_DEBUG);
        self.uniforms.write(g, self.built.layouts[1], sets.len() as u32, sets, moved, |buf, d| {
            let u = uniform_of(uniforms, d);
            buf[..16].copy_from_slice(bytes(&[u[0], u[1], u[2], 0.0]));
        });
    }

    /// `depth` is the world's, which nothing reads after this.
    #[allow(clippy::too_many_arguments)]
    fn draw(&self, g: &Gpu, encoder: u32, canvas: u32, depth: u32, draws: &Draws, blocks: &Blocks, stamp: &mut Stamp) {
        let grid = draws.of(PIPELINE_CAMERA_GRID).len() > 0;
        let held = self.uniforms.slots().filter(|_| draws.of(PIPELINE_DEBUG).len() > 0);
        if held.is_none() && !grid {
            return;
        }
        let slots = held.as_ref();
        // Each set held: its uniform's slot, its line vertices and its face vertices.
        let sets = || {
            draws.of(PIPELINE_DEBUG).enumerate().filter_map(|(k, d)| {
                let slots = slots?;
                let (block, _, _) = blocks.get(d[0])?;
                (d[1] > 0).then_some((k, d, block, d[1], d[2], slots))
            })
        };
        // With no faces the lines' pass clears the depth itself.
        let faced = sets().any(|s| s.4 > 0);
        if faced {
            let pass = g.pass(encoder, &pass_desc(0, Load::Clear, depth, Load::Clear, 0.0, stamp.pass()));
            g.set_pipeline(pass, self.faces.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, block, lines, faces, slots) in sets().filter(|s| s.4 > 0) {
                let (group, offset) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[offset]);
                g.set_vertex_buffer(pass, 0, block, lines * 28, faces * 12);
                g.draw(pass, faces, 1, 0, 0);
            }
            g.end(pass);
        }
        // A set's faces hide only its own lines: the faced sets' lines over
        // their faces' depth, then the rest over none, so the focus's frame
        // is whole under the coverage laid on the ground.
        let depth_load = if faced { Load::Load } else { Load::Clear };
        let mut pass = g.pass(encoder, &pass_desc(canvas, Load::Load, depth, depth_load, 0.0, stamp.pass()));
        for faces in [true, false] {
            if faced && !faces {
                g.end(pass);
                pass = g.pass(encoder, &pass_desc(canvas, Load::Load, depth, Load::Clear, 0.0, stamp.pass()));
            }
            g.set_pipeline(pass, self.built.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, block, lines, _, slots) in sets().filter(|s| (s.4 > 0) == faces) {
                let (group, offset) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[offset]);
                g.set_vertex_buffer(pass, 0, block, 0, lines * 28);
                // A segment an instance: its two vertices, read as one.
                g.draw(pass, 6, lines / 2, 0, 0);
            }
        }
        if grid {
            g.set_pipeline(pass, self.grid.pipeline);
            g.set_bind_group(pass, 0, self.grid_group, &[]);
            g.draw(pass, 3, 1, 0, 0);
        }
        g.end(pass);
    }
}

/// The eye's rays, each drawn at its own pixel where it met a face, painted
/// by paint.wgsl's `paint` (pxl-spec-core ray.rs), from the world surface's
/// `rays` the GPU's cast leaves (`Cast`).
struct RaysPass {
    built: Built,
    /// The same pixels' depth alone, into the occlusion's prepass.
    depth: Built,
    /// Its group 0 over the lit pass's entries and what it binds besides
    /// (`bound`, the surface's rays and the cast's eye): made again exactly
    /// when one of them is another.
    group: u32,
    bound: [u32; 2],
    /// The lit pass's entries it paints by (`Renderer::lit_entries`); none
    /// until the renderer hands them.
    lit: Vec<Entry>,
    looks: u32,
    ground: Option<GroundBuffers>,
    cast: Option<CastBuffers>,
    /// What this frame does with the cast, as `prepare` decided it: nothing
    /// cast (`plain`), the rays standing (`standing`), or a cast run
    /// (`cast`). `dispatch` and the frame's report read it.
    kind: FrameKind,
}

/// The terrain's frames on the GPU and the passes that draw their cells.
struct GroundBuffers {
    /// Per square of the windows, its columns, levels, runs and cells as
    /// laid, which `heights_cs`, `ways_cs` and `bound_cs` write in the turn
    /// it is handed (pxl-spec-core `window::SQUARE_WORDS`).
    ground: u32,
    /// Per square, its cells' four looks (`LOOKS_BYTES`).
    looks: u32,
    /// The squares the turn handed, `window::HANDED` at most, a record each
    /// (`window::HANDED_WORDS`): its words, then its looks, its tile's
    /// samples and its lines, which only its fill reads; `heights_cs`'s,
    /// `bound_cs`'s and `ways_cs`'s one per `id.z`.
    fresh: u32,
    fresh_len: u32,
    /// Per square place (`window::SQUARES`), its cells' way back: the fill
    /// the next landing must be of (its ticket 0 where none is asked).
    asked: [CellsBack; SQUARES as usize],
    /// The squares the frame handed, in the order handed, each with a read
    /// of its own (`hand` hands none past the reads free): a fill's records
    /// live in its frame's scratch, so its read is taken in its frame.
    handed: [u32; HANDED as usize],
    /// The buffers cells are copied into and mapped back through, as many as
    /// a frame hands squares a mapping still out a frame more (`READS`);
    /// each one's square and the fill it carries (`FREE` where none), and
    /// its mapping's own word (`read_back`), which the host writes: held
    /// where the guest's allocator puts it, inside the region the host
    /// addresses, as an arriving image's word is. A static is outside it on
    /// a device.
    reads: [u32; READS],
    held: [(u32, u32); READS],
    mapping: Box<[u32; READS]>,
    /// Each read's records, mapped beside its square's words into its room
    /// on the heap, as many bytes as the room holds, the buffer of its size
    /// (none before its first read), and its mapping's word: a read lands
    /// with both. Every room is made `record_bytes` as its read is next
    /// taken, the heap granting it; else it keeps the room it has.
    record_reads: [u32; READS],
    record_rooms: [Box<[u32]>; READS],
    record_mapping: Box<[u32; READS]>,
    record_bytes: u32,
    /// What each read's square was built in, indexed by `Short`: the pool's
    /// blocks and the blocks a square's scratch held (`ran_out`).
    built_in: [[u32; 2]; READS],
    /// Whether a build ran out of the pool, or of the scratch, since it last
    /// grew, indexed by `Short` (`grow`, `grow_scratch`).
    ran_out: [bool; 2],
    /// The ways' pool, which the cast reads, its blocks and its free stack;
    /// and the handed squares' scratch, the blocks each square's holds a
    /// tree of (`ways::tree::scratch_words`), whose records each read copies
    /// in the frame they are built: nothing in it outlives its frame.
    pool: u32,
    blocks: u32,
    stack: u32,
    scratch: u32,
    scratch_blocks: u32,
    /// Each slot's number, `bound_cs`'s level and `ways_cs`'s pass, at a
    /// dynamic offset of their groups.
    slots: u32,
    heights_built: Built,
    heights_group: u32,
    bound_built: Built,
    /// `bound_cs`'s group, its level at a dynamic offset (`SLOT_STRIDE`).
    bound_group: u32,
    /// `ways_cs`'s, and its group, its pass at a dynamic offset.
    ways_built: Built,
    ways_group: u32,
    /// A re-filled square's old ways' tree given back, its new one built,
    /// the blocks the build gave back pushed (pxl-spec-core `ways::tree`):
    /// each pass and its group, over the ways' pool, heap words then data
    /// words (`pool_bytes`), its free stack, the count taken then the ids,
    /// block 0 never among them, a word a block, and each handed square's
    /// scratch past its build (`ways::tree::FREED_AT`).
    free_built: Built,
    free_group: u32,
    tree_built: Built,
    tree_group: u32,
    freed_built: Built,
    freed_group: u32,
}

/// The ways' pool of `blocks` on the GPU, in bytes: a slot two words,
/// eight a block. Its free stack is a word a block: the count taken, then
/// every block's id but block 0's, every block free but block 0 as WebGPU
/// makes it, zeroed (pxl-shader `TAKEN`, `id`), so nothing is written to
/// start it, nor to grow it.
const fn pool_bytes(blocks: u32) -> u32 {
    blocks * ways::tree::BLOCK_WORDS * 4
}
const _: () = assert!(
    crate::frame::shader::floor::fits(pool_bytes(ways::tree::POOL_BLOCKS) as u64, true),
    "the ways' pool at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The ways' pool of `blocks` and its free stack, zeroed: each copied
/// from as it grows, and into.
fn pool_buffers(g: &Gpu, blocks: u32) -> (u32, u32) {
    let usage = buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST;
    (g.buffer(pool_bytes(blocks), usage), g.buffer(word_bytes(blocks as usize), usage))
}

/// The handed squares' scratch, each square's for a tree of `blocks`, in
/// bytes: `HANDED` of them, every square's alike, so each finds its own off
/// the buffer's length (pxl-spec-core fill.rs `scratch_at`).
const fn scratch_bytes(blocks: u32) -> u32 {
    HANDED * ways::tree::scratch_words(blocks) * 4
}
const _: () = assert!(
    crate::frame::shader::floor::fits(scratch_bytes(ways::tree::SQUARE_BLOCKS) as u64, true),
    "the scratch at its most binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The handed squares' scratch, each square's for a tree of `blocks`,
/// which its records are copied out of.
fn scratch_buffer(g: &Gpu, blocks: u32) -> u32 {
    g.buffer(scratch_bytes(blocks), buffer_usage::STORAGE | buffer_usage::COPY_SRC)
}

/// What a build may run out of, each grown as builds do: the ways' pool
/// (`grow`) and the handed squares' scratch (`grow_scratch`), an index of
/// `GroundBuffers::built_in` and `ran_out`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Short {
    Pool,
    Scratch,
}

/// What a build whose records' count is `n` ran out of (pxl-shader
/// `ways_tree_cs`): none counted the pool, `CRAMPED` the scratch; neither
/// for a tree built, nor for one past the square's own most (`NONE`), which
/// no growth lets it build.
fn short_of(n: u32) -> Option<Short> {
    match n {
        0 => Some(Short::Pool),
        ways::tree::CRAMPED => Some(Short::Scratch),
        _ => None,
    }
}

/// What a build that ran out of the pool or the scratch does, built in one
/// of `built` blocks, the one it ran out of now of `blocks`, at most `most`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RanOut {
    /// It grew since: built again, as the larger may hold it.
    Again,
    /// The one it ran out of, which grows (`grow`, `grow_scratch`), and
    /// built again.
    Grow,
    /// It ran out at its most: refused.
    Refused,
}

/// A read is out a frame more than its build (`READS`), so a build that
/// ran out of the pool or the scratch may land after it grew for another:
/// judged against what it was built in, it grows it no further and is
/// refused only by it at its most. The pool's and the scratch's alike,
/// each against its own (`Short`).
fn ran_out(built: u32, blocks: u32, most: u32) -> RanOut {
    if built < blocks {
        RanOut::Again
    } else if blocks < most {
        RanOut::Grow
    } else {
        RanOut::Refused
    }
}

impl GroundBuffers {
    /// The ways' passes' groups over the pool, its stack and the scratch as
    /// they are, and `bound_cs`'s and `ways_cs`'s over the scratch.
    fn ways_groups(&mut self, g: &Gpu) {
        for group in [self.bound_group, self.ways_group, self.free_group, self.tree_group, self.freed_group] {
            release(group);
        }
        let (fresh, ground, pool, stack, scratch) = (self.fresh, self.ground, self.pool, self.stack, self.scratch);
        let entries = [buffer_range_entry(0, self.slots, 0, 16), buffer_entry(1, fresh), buffer_entry(2, ground), buffer_entry(3, scratch)];
        self.bound_group = g.bind_group(self.bound_built.layouts[0], &entries);
        self.ways_group = g.bind_group(self.ways_built.layouts[0], &entries);
        self.free_group = g.bind_group(
            self.free_built.layouts[0],
            &[buffer_entry(0, fresh), buffer_entry(1, ground), buffer_entry(2, pool), buffer_entry(3, stack)],
        );
        self.tree_group = g.bind_group(
            self.tree_built.layouts[0],
            &[buffer_entry(0, fresh), buffer_entry(1, ground), buffer_entry(2, pool), buffer_entry(3, stack), buffer_entry(4, scratch)],
        );
        self.freed_group = g.bind_group(self.freed_built.layouts[0], &[buffer_entry(0, scratch), buffer_entry(1, stack)]);
    }

    /// The ways' pool twice as large, never past `most` blocks, once a
    /// build ran out of it (`read_back`): its heap words and its stack
    /// copied where they were, its data words to the new data's first, so
    /// every id a tree or a read in flight names holds what it held, and the
    /// stack's slots past the old hold the new blocks free (pxl-shader
    /// `TAKEN`). Ahead of the turn's squares, so their builds and the cast
    /// read the new one; the cast's groups follow its id (`prepare`).
    fn grow(&mut self, g: &Gpu, most: u32) {
        let blocks = (2 * self.blocks).min(most).min(ways::tree::POOL_BLOCKS);
        if blocks <= self.blocks {
            return;
        }
        let (pool, stack) = pool_buffers(g, blocks);
        let half = |blocks: u32| pool_bytes(blocks) / 2;
        let e = g.encoder();
        g.copy_buffer(e, (self.pool, 0), (pool, 0), half(self.blocks));
        g.copy_buffer(e, (self.pool, half(self.blocks)), (pool, half(blocks)), half(self.blocks));
        g.copy_buffer(e, (self.stack, 0), (stack, 0), word_bytes(self.blocks as usize));
        g.submit(e);
        release(self.pool);
        release(self.stack);
        (self.pool, self.stack, self.blocks) = (pool, stack, blocks);
        self.ways_groups(g);
    }

    /// The scratch's squares each twice as large, never past `most` blocks,
    /// once a build outgrew it (`read_back`). Nothing is copied: what a
    /// square's holds goes with the frame it is filled in, its records
    /// copied into its read in that frame (`read_back`), before any turn
    /// grows it, and each read carries its own buffer. Ahead of the turn's
    /// squares, so their passes lay theirs out by the new length.
    fn grow_scratch(&mut self, g: &Gpu, most: u32) {
        let blocks = (2 * self.scratch_blocks).min(most).min(ways::tree::SQUARE_BLOCKS);
        if blocks <= self.scratch_blocks {
            return;
        }
        let scratch = scratch_buffer(g, blocks);
        release(self.scratch);
        (self.scratch, self.scratch_blocks) = (scratch, blocks);
        self.ways_groups(g);
    }

    /// Read `k`'s records' room, free, made `record_bytes` where it is
    /// another, its buffer with it, the old room given back first, so no
    /// two of a read's are held at once; where the heap refuses it, a room
    /// of the count alone, whose read lands short and is filled again, as a
    /// square waits on the heap. A read's first room is made here.
    fn room(&mut self, g: &Gpu, k: usize) {
        let words = (self.record_bytes / 4) as usize;
        if self.record_rooms[k].len() == words {
            return;
        }
        self.record_rooms[k] = Box::default();
        let mut room = Vec::new();
        if room.try_reserve_exact(words).is_err() {
            room = vec![0];
        } else {
            room.resize(words, 0);
        }
        release(self.record_reads[k]);
        self.record_reads[k] = g.buffer(word_bytes(room.len()), buffer_usage::MAP_READ | buffer_usage::COPY_DST);
        self.record_rooms[k] = room.into_boxed_slice();
    }
}

/// What the GPU's cast reads; it writes the world surface's rays.
struct CastBuffers {
    built: Built,
    /// The camera grid's beam rays (`beam_cs`, a thread a corner), where
    /// each cell's pixels start off them (`cells_cs`, a thread a cell of
    /// the screen), the samples walked from those starts (`sample_cs`, a
    /// thread a sample), and their buffer, over the cast's group
    /// (pxl-spec-core `beam`): a line a corner, a start a cell, a stop a
    /// sample, as the eye's words lay the frame's `grid`, none a pixel's;
    /// room for `beam_room` words
    /// (doc/minutes/20261022-a-cell-casts-a-beam.md).
    beam_built: Built,
    cells_built: Built,
    sample_built: Built,
    beams: u32,
    grid: beam::Grid,
    beam_room: u32,
    /// The observed pyramid (pxl-spec-core `observed`): its buffer, no
    /// pixel's; the pixels the cast's reading walk missed walked again
    /// (`miss_cs`, a thread a drawn pixel), each told by the mark its own
    /// first ray word holds (pxl-spec-core `ray::MISS`); the pyramid forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's windows' slots that moved
    /// (`moved`, a bit a slot, against the slots as `slots` last held them);
    /// as this frame's cast does (`forgets`, pxl-spec-core
    /// `observed::FORGETS`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    miss_built: Built,
    forget_built: Built,
    seen: u32,
    frame: u32,
    forget: bool,
    moved: [u32; observed::MOVED_WORDS as usize],
    slots: Vec<i32>,
    forgets: u32,
    cuts: [u32; eye::CUTS],
    /// The eye's windows folded over their squares (`joined_cs`), over its
    /// group, the window buffer's rows past their slot table
    /// (`window::JOINED_AT`), written where the windows' slots moved
    /// (`refold`); then copied into `joined_read` and mapped back
    /// (`read_back`), once a mapping is free, while `joined_back`; the
    /// mapping's own word where the guest's allocator puts it, as a
    /// square's is.
    joined_built: Built,
    /// The world's depth from the sun, a texel of the sun's map each
    /// (`sun_cs`), cast with the eye's rays where the shadows are on.
    sun_built: Built,
    sun_mark_built: Built,
    /// The sun's mark's group and its cast's (`sun_groups`), and the
    /// buffers they bind: made again exactly when one of them is another.
    sun_groups: [u32; 2],
    sun_bound: [u32; 7],
    /// The tiles the mark lists, and the cast's dispatch over them
    /// (pxl-spec-core `sun::LIST_AT`).
    sun_list: u32,
    /// What the sun's cast last read beside the eye (`Read`): the light's
    /// frame (`Shadow::frame`) and whether the world under it moved past
    /// the nodes it names, where its clipmap keeps nothing; the eye's
    /// place, quality and zooms and the light box, where it is cast keeping
    /// what stands (`sun_at`); the box's corner the clipmap holds the world at;
    /// and whether this frame casts it.
    sun_read: Read,
    sun_at: Read,
    sun_kept: Option<[i32; 2]>,
    /// The nodes the world's query answers otherwise since the sun's map was
    /// last cast, as its cast binds them (pxl-spec-core `sun::MOVED_WORDS`).
    sun_moved: u32,
    sun_cast: bool,
    joined_read: u32,
    joined_mapping: Box<u32>,
    refold: bool,
    joined_back: bool,
    /// Its group 0, and the buffers it binds (`cast_group`): made again
    /// exactly when one of them is another.
    group: u32,
    bound: [u32; 10],
    eye: u32,
    window: u32,
    shards: u32,
    /// Shards the buffer holds.
    shard_room: usize,
    /// The drawn pixels it casts, a ray each: the world surface's.
    size: [u32; 2],
    /// The overwrites' heap lane then their data lane, each `overwrite_room`
    /// words, the lanes' own room; the revision it holds, none at first.
    overwrites: u32,
    overwrite_room: usize,
    overwrite_revision: Option<u32>,
    grown: Grown,
    /// What the last cast read (`Read`), and whether this frame's is the
    /// same: the rays and the decor grown over them stand, and nothing is
    /// cast.
    read: Read,
}

/// The nodes the world's query answers otherwise this frame, as the sun's
/// cast binds them (pxl-spec-core `sun::MOVED_WORDS`): their count, then
/// each X, Y and its zoom; past `sun::MOVED`, or a slot with no square held
/// before, `whole`, and the world is cast again whole.
struct Nodes {
    words: [u32; sun::MOVED_WORDS],
    len: usize,
    whole: bool,
}

impl Default for Nodes {
    fn default() -> Nodes {
        Nodes {
            words: [0; sun::MOVED_WORDS],
            len: 0,
            whole: false,
        }
    }
}

impl Nodes {
    fn push(&mut self, [x, y]: [i32; 2], zoom: u32) {
        let node = [x as u32, y as u32, zoom, 0];
        if (0..self.len).any(|k| self.words[4 + 4 * k..8 + 4 * k] == node) {
            return;
        }
        if self.len == sun::MOVED {
            self.whole = true;
            return;
        }
        self.words[4 + 4 * self.len..8 + 4 * self.len].copy_from_slice(&node);
        self.len += 1;
        self.words[0] = self.len as u32;
    }
}

/// A square on the GPU, in bytes.
const SQUARE_BYTES: u32 = SQUARE_WORDS * 4;

/// The windows' squares on the GPU, in bytes: one storage buffer, bound
/// whole by the cast's, the levels' and the ways' groups.
const GROUND_BYTES: u64 = SQUARES as u64 * SQUARE_BYTES as u64;
const _: () = assert!(
    crate::frame::shader::floor::fits(GROUND_BYTES, true),
    "GROUND_BYTES: the windows' squares bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// The windows' squares' cells' four looks, a word a cell in the squares'
/// order: written by the fill off each handed square (`heights_cs`), read
/// by the cast at a hit (pxl-shader `World`'s `Looks`). Their own
/// buffer, as the squares' binding has no room for them.
const LOOKS_BYTES: u64 = SQUARES as u64 * SQUARE_CELLS as u64 * 4;
const _: () = assert!(
    crate::frame::shader::floor::fits(LOOKS_BYTES, true),
    "LOOKS_BYTES: the squares' looks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// `bound_cs`'s and `ways_cs`'s one group: its level or its pass at a
/// dynamic offset (`SLOT_STRIDE`), the fresh frames, the frames it writes
/// the levels of, and the handed squares' scratch it writes or reads the
/// runs in. The cast's and `joined_cs`'s one group is the cast's, which
/// binds all of theirs.
const BOUND_GROUP: [u32; compute::BOUND_GROUP.len()] = dynamic(compute::BOUND_GROUP, 0);
const _: () = assert!(same(compute::BOUND_GROUP, compute::WAYS_GROUP), "the levels' and the ways' passes share a group");
const _: () = assert!(compute::WAYS_WORKGROUP[0] == pxl_spec_core::fill::SCAN_THREADS, "a square's columns are summed by one workgroup");
const _: () = assert!(
    within(compute::JOINED_GROUP, compute::MISS_GROUP)
        && within(compute::BEAM_GROUP, compute::MISS_GROUP)
        && within(compute::CELLS_GROUP, compute::MISS_GROUP)
        && within(compute::CAST_GROUP, compute::MISS_GROUP)
        && within(compute::FORGET_GROUP, compute::MISS_GROUP),
    "the fold, the beam, the cells, the reading cast and the forgetting bind the group the missed pixels' cast binds whole"
);

/// The cast's one group, every kernel's over the cast's world bar the
/// sun's: the one the missed pixels' cast binds whole. The sun's cast binds
/// its world and its clipmap (`SUN_GROUP`), past the floor's storage
/// buffers were they one group.
const CAST_GROUP: [u32; compute::MISS_GROUP.len()] = compute::MISS_GROUP;

/// A number's slot in the fill's numbers, read at a dynamic offset: every
/// device's dynamic offset alignment.
const SLOT_STRIDE: u32 = UNIFORM_ALIGN_MAX;

impl RaysPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines) -> RaysPass {
        let built = pipelines.build(g, PIPELINE_SCREEN_RAYS, g.canvas());
        let depth = pipelines.build(g, PIPELINE_SCREEN_PREPASS, Format::depth32float);
        let words = look_words();
        let looks = g.buffer(word_bytes(4 * words.len()), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(looks, 0, bytes(&words));
        RaysPass {
            built,
            depth,
            group: 0,
            bound: [0; 2],
            lit: Vec::new(),
            looks,
            ground: None,
            cast: None,
            kind: FrameKind::plain,
        }
    }

    /// The lit pass's entries, handed again whenever one of its objects is
    /// made again: the group is made again over them when next drawn.
    fn lit(&mut self, entries: Vec<Entry>) {
        self.lit = entries;
        self.bound = [0; 2];
    }

    /// The group over `bound`, the surface's rays and the cast's eye, made
    /// again where either is another or the lit pass's entries were handed.
    fn regroup(&mut self, g: &Gpu, bound: [u32; 2]) {
        if bound == self.bound || self.lit.is_empty() {
            return;
        }
        release(self.group);
        let mut e = self.lit.clone();
        e.push(view_entry(3, bound[0]));
        e.push(buffer_entry(20, self.looks));
        e.push(buffer_entry(21, bound[1]));
        self.group = g.bind_group(self.built.layouts[0], &e);
        self.bound = bound;
    }

    /// Each held tile the GPU does not yet hold handed to its frame, copied
    /// into the turn's stream as it has room, the packets they live in let
    /// go once handed; whether or not an eye casts, as a tile's cells are
    /// no eye's.
    fn hand(&mut self, g: &Gpu, pipelines: &mut Pipelines, ground: &mut GroundSquares) {
        let blocks = ground.pool[0].min(ground.pool[1]).min(ways::tree::POOL_BLOCKS);
        let scratch_blocks = ground.scratch[0].min(ground.scratch[1]).clamp(1, ways::tree::SQUARE_BLOCKS);
        let gb = self.ground.get_or_insert_with(|| {
            let buffer = |size: u32, usage: u32| g.buffer(size, usage);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            let frames_buffer = buffer(GROUND_BYTES as u32, storage | buffer_usage::COPY_SRC);
            let fresh = buffer(word_bytes((HANDED_WORDS * HANDED) as usize), storage);
            let looks = buffer(LOOKS_BYTES as u32, storage);
            let heights_built = pipelines.build_compute(g, SHADER_HEIGHTS, &compute::HEIGHTS_GROUP);
            let heights_group = g.bind_group(heights_built.layouts[0], &[buffer_entry(0, fresh), buffer_entry(1, frames_buffer), buffer_entry(2, looks)]);
            // Each slot's number in it, written once: `bound_cs`'s level,
            // `ways_cs`'s pass.
            let slots = buffer((LEVELS + 1) * SLOT_STRIDE, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            for k in 0..=LEVELS {
                g.write_buffer(slots, k * SLOT_STRIDE, bytes(&[k, 0, 0, 0]));
            }
            let scratch = scratch_buffer(g, scratch_blocks);
            let (bound_built, ways_built) = (pipelines.build_compute(g, SHADER_BOUND, &BOUND_GROUP), pipelines.build_compute(g, SHADER_WAYS, &BOUND_GROUP));
            let (pool, stack) = pool_buffers(g, blocks);
            let free_built = pipelines.build_compute(g, SHADER_WAYS_FREE, &compute::WAYS_FREE_GROUP);
            let tree_built = pipelines.build_compute(g, SHADER_WAYS_TREE, &compute::WAYS_TREE_GROUP);
            let freed_built = pipelines.build_compute(g, SHADER_WAYS_FREED, &compute::WAYS_FREED_GROUP);
            let mut gb = GroundBuffers {
                ground: frames_buffer,
                looks,
                fresh,
                fresh_len: 0,
                asked: [CellsBack::default(); SQUARES as usize],
                handed: [0; HANDED as usize],
                reads: core::array::from_fn(|_| buffer(BACK_WORDS * 4, buffer_usage::MAP_READ | buffer_usage::COPY_DST)),
                held: [(FREE, 0); READS],
                mapping: Box::new([0; READS]),
                record_reads: [0; READS],
                record_rooms: core::array::from_fn(|_| Box::default()),
                record_mapping: Box::new([0; READS]),
                built_in: [[0; 2]; READS],
                record_bytes: (ground.records.next_multiple_of(4)).clamp(word_bytes(1 + ways::tree::RECORD_WORDS as usize), ways::tree::RECORDS_BYTES),
                pool,
                blocks,
                stack,
                ran_out: [false; 2],
                scratch,
                scratch_blocks,
                slots,
                heights_built,
                heights_group,
                bound_built,
                bound_group: 0,
                ways_built,
                ways_group: 0,
                free_built,
                free_group: 0,
                tree_built,
                tree_group: 0,
                freed_built,
                freed_group: 0,
            };
            gb.ways_groups(g);
            gb
        });
        if core::mem::take(&mut gb.ran_out[Short::Pool as usize]) {
            gb.grow(g, ground.pool[1]);
        }
        if core::mem::take(&mut gb.ran_out[Short::Scratch as usize]) {
            gb.grow_scratch(g, ground.scratch[1]);
        }
        // A square handed takes a read its frame: none past those free.
        let free = gb.held.iter().filter(|h| h.0 == FREE).count().min(HANDED as usize) as u32;
        let mut fresh = 0;
        (ground.upload)(&mut |q| {
            if fresh == free || !room(write_buffer_bytes(q.looks) + write_buffer_bytes(bytes(q.samples)) + write_buffer_bytes(bytes(q.lines)) + write_buffer_bytes(bytes(&q.fresh))) {
                return false;
            }
            let (square, at) = (q.fresh[window::FRESH_SQUARE as usize], window::handed_at(fresh) * 4);
            g.write_buffer(gb.fresh, at, bytes(&q.fresh));
            g.write_buffer(gb.fresh, at + LOOKS_AT * 4, q.looks);
            g.write_buffer(gb.fresh, at + ELE_AT * 4, bytes(q.samples));
            g.write_buffer(gb.fresh, at + LINES_AT * 4, bytes(q.lines));
            gb.asked[square as usize] = q.back;
            gb.handed[fresh as usize] = square;
            fresh += 1;
            true
        });
        gb.fresh_len = fresh;
    }

    /// The cast pass's pipelines and buffers, made once, where the frame
    /// first casts: before any of its uploads (`hand`), which take the
    /// stream only while `stream::RESERVE` is left after them, so the
    /// shaders made here are never left without room.
    fn make(&mut self, g: &Gpu, pipelines: &mut Pipelines) {
        if self.cast.is_some() {
            return;
        }
        // One group for every kernel over the cast's world.
        let group = &CAST_GROUP;
        let built = pipelines.build_compute(g, SHADER_CAST, group);
        let beam_built = pipelines.build_compute(g, SHADER_BEAM, group);
        let cells_built = pipelines.build_compute(g, SHADER_CELLS, group);
        let sample_built = pipelines.build_compute(g, SHADER_SAMPLE, group);
        let joined_built = pipelines.build_compute(g, SHADER_JOINED, group);
        let miss_built = pipelines.build_compute(g, SHADER_MISS, group);
        let forget_built = pipelines.build_compute(g, SHADER_FORGET, group);
        let sun_built = pipelines.build_compute(g, SHADER_SUN, &compute::SUN_GROUP);
        let sun_mark_built = pipelines.build_compute(g, SHADER_SUN_MARK, &compute::SUN_MARK_GROUP);
        let buffer = |size: u32, usage: u32| g.buffer(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            built,
            beam_built,
            cells_built,
            sample_built,
            beams: 0,
            grid: beam::Grid::of_words([0.0; 2], [0; eye::GRID_WORDS]),
            beam_room: 0,
            miss_built,
            forget_built,
            seen: buffer(word_bytes(observed::WORDS as usize), storage),
            frame: 0,
            forget: true,
            moved: [0; observed::MOVED_WORDS as usize],
            slots: Vec::new(),
            forgets: observed::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            joined_built,
            sun_built,
            sun_mark_built,
            sun_groups: [0; 2],
            sun_bound: [0; 7],
            sun_list: buffer(word_bytes(sun::LIST_AT + (shadow::SIZE / sun::TILE).pow(2) as usize), storage | buffer_usage::INDIRECT),
            sun_read: Read::default(),
            sun_at: Read::default(),
            sun_kept: None,
            sun_moved: buffer(word_bytes(sun::MOVED_WORDS), buffer_usage::UNIFORM | buffer_usage::COPY_DST),
            sun_cast: false,
            joined_read: buffer(word_bytes(window::EYE_JOINED_WORDS as usize), buffer_usage::MAP_READ | buffer_usage::COPY_DST),
            joined_mapping: Box::new(DONE),
            refold: false,
            joined_back: false,
            group: 0,
            bound: [0; 10],
            eye: buffer(word_bytes(eye::WORDS), buffer_usage::UNIFORM | buffer_usage::COPY_DST),
            window: buffer(word_bytes((4 * window::JOINED_AT + window::EYE_JOINED_WORDS) as usize), storage | buffer_usage::COPY_SRC),
            shards: 0,
            shard_room: 0,
            size: [0; 2],
            overwrites: 0,
            overwrite_room: 0,
            overwrite_revision: None,
            grown: Grown::new(g, pipelines),
            read: Read::default(),
        };
        self.cast = Some(c);
    }

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels), and the world's depth from the sun into `sun`'s
    /// view: the GPU's cast where an eye casts, its eye handed over first.
    fn prepare(&mut self, g: &Gpu, cast: Option<Cast>, meshes: &[u32; GROUPS as usize], rays: (u32, [u32; 2]), sun_map: (u32, u32)) {
        // No cast, no world: no ray cast, none drawn and no decor grown.
        // What the cast reads stands for the next: the ground it binds is
        // made once.
        let (Some(mut cast), Some((ground, looks, pool))) = (cast, self.ground.as_ref().map(|gb| (gb.ground, gb.looks, gb.pool))) else {
            self.kind = FrameKind::plain;
            if let Some(c) = self.cast.as_mut() {
                c.grown.off();
                c.read.forget();
                c.sun_read.forget();
                c.sun_cast = false;
            }
            return;
        };
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The cast reads exactly this, all of it here: a field the frame hands
        // the cast fails to destructure until it is read. The overwrites'
        // contents are their revision's. The decor it grows reads its own
        // (`Grown::prepare`), so a change to decor alone casts no ray again.
        let Cast {
            eye,
            window,
            shards,
            overwrites:
                Overwrites {
                    heap: _,
                    data: _,
                    room,
                    revision,
                    since: _,
                    written: _,
                },
            decor: _,
        } = &cast;
        let r = &mut c.read;
        r.begin();
        // Each lane written again where it moved; the sun's words its cast's
        // alone (below).
        let eye_moved = r.read(&eye[..eye::SUN]);
        let window_moved = r.read(window);
        let shards_moved = r.read(shards);
        // A surface made again holds no rays.
        r.read(&[*room as u32, *revision, rays.0]);
        // And the ground's squares, written this frame or not.
        let fresh = self.ground.as_ref().map_or(0, |gb| gb.fresh_len);
        self.kind = if r.end() && fresh == 0 && c.group != 0 { FrameKind::standing } else { FrameKind::cast };
        // The slot table fills the rows before the fold's (`window::JOINED_AT`).
        xpute_kit::ensure!(cast.window.len() == (4 * window::JOINED_AT) as usize, ENOTRECOVERABLE, cast.window.len());
        if window_moved {
            g.write_buffer(c.window, 0, bytes(cast.window));
            c.refold = true;
        }
        c.size = rays.1;
        // The grid as the eye's words lay it (pxl-spec-core `eye::GRID`), the
        // dispatches' sizes, and room for its corners' lines, its cells'
        // starts and its samples' stops, made again only where they outgrow
        // it.
        let tan = [f32::from_bits(cast.eye[eye::TAN]), f32::from_bits(cast.eye[eye::TAN + 1])];
        c.grid = beam::Grid::of_words(tan, cast.eye[eye::GRID..eye::GRID + eye::GRID_WORDS].try_into().or_bug(Errno::ENOTRECOVERABLE));
        let need = c.grid.size();
        if need > c.beam_room {
            if c.beams != 0 {
                release(c.beams);
            }
            let size = word_bytes(need as usize);
            xpute_kit::ensure!(crate::frame::shader::floor::fits(size as u64, true), ENOTRECOVERABLE, need);
            (c.beams, c.beam_room) = (g.buffer(size, buffer_usage::STORAGE | buffer_usage::COPY_DST), need);
        }
        let shards_grow = cast.shards.len() > c.shard_room;
        if shards_grow {
            if c.shards != 0 {
                release(c.shards);
            }
            c.shard_room = cast.shards.len();
            c.shards = g.buffer(word_bytes(4 * c.shard_room), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        }
        if shards_moved || shards_grow {
            // Read where the kernel holds it, one record however large.
            g.upload_buffer(c.shards, 0, bytes(cast.shards));
        }
        // Sized by the lanes' own room, so it is made again only when they grow.
        let o = &cast.overwrites;
        let overwrites_grow = o.room != c.overwrite_room;
        if overwrites_grow {
            if c.overwrites != 0 {
                release(c.overwrites);
            }
            c.overwrite_room = o.room;
            c.overwrites = g.buffer(word_bytes(2 * o.room), buffer_usage::STORAGE | buffer_usage::COPY_DST);
            c.overwrite_revision = None;
        }
        let overwrites_moved = c.overwrite_revision != Some(o.revision);
        if overwrites_moved {
            // Held at `since`: the blocks written after it alone, a record
            // each, where the stream has room for them all; else both lanes
            // whole, read where they live, two records however large.
            fn block(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..first as usize + 8]
            }
            let listed = |lane: &[u32]| o.written.iter().map(|&f| write_buffer_bytes(bytes(block(lane, f)))).sum::<usize>();
            if c.overwrite_revision == Some(o.since) && crate::gpu::api::room(listed(o.heap) + listed(o.data)) {
                for &first in o.written {
                    g.write_buffer(c.overwrites, word_bytes(first as usize), bytes(block(o.heap, first)));
                    g.write_buffer(c.overwrites, word_bytes(o.room + first as usize), bytes(block(o.data, first)));
                }
            } else {
                g.upload_buffer(c.overwrites, 0, bytes(o.heap));
                g.upload_buffer(c.overwrites, word_bytes(o.room), bytes(o.data));
            }
            c.overwrite_revision = Some(o.revision);
        }
        // What the world's query reads moved: the windows, the squares the
        // turn handed (their cells, looks and ways), the service area, what
        // is placed, a preview's cuts. What the observed pyramid kept is
        // forgotten; so it is once its frames would outrun a key's tag.
        let cuts: [u32; eye::CUTS] = cast.eye[eye::CARVE..eye::CARVE + eye::CUTS].try_into().or_bug(Errno::ENOTRECOVERABLE);
        c.forget |= shards_moved || shards_grow || overwrites_moved || cuts != c.cuts || c.frame >= observed::FRAMES;
        // The eye's windows' slots that hold another square, or whose square
        // the turn handed: what is over them is asked again; the sun's map
        // casts again what passes over the square they held and the one they
        // hold, and over a preview's cuts where they moved.
        let mut nodes = Nodes::default();
        for at in [0, eye::CUT_WORDS] {
            let (was, is) = (&c.cuts[at..at + eye::CUT_WORDS], &cuts[at..at + eye::CUT_WORDS]);
            if was != is {
                for cut in [was, is] {
                    if cut[3] != eye::NO_CUT {
                        nodes.push([cut[0] as i32, cut[1] as i32], cut[3]);
                    }
                }
            }
        }
        c.cuts = cuts;
        let eye_rows = (window::EYE_WINDOWS * window::SLOTS) as usize;
        let square = |row: &[i32], k: usize| ([row[0], row[1]], window::squares_zoom(window::eye_cell((k / window::SLOTS as usize) as u32)));
        if window_moved {
            let rows = &cast.window[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.slots.get(4 * k..4 * k + 4);
                if was != Some(row) {
                    c.moved[k / 32] |= 1 << (k % 32);
                    match was {
                        Some(was) => nodes.push(square(was, k).0, square(was, k).1),
                        None => nodes.whole = true,
                    }
                    nodes.push(square(row, k).0, square(row, k).1);
                }
            }
            c.slots.clear();
            c.slots.extend_from_slice(rows);
        }
        if let Some(gb) = self.ground.as_ref() {
            for &s in &gb.handed[..fresh as usize] {
                let k = s as usize;
                if k < eye_rows {
                    c.moved[k / 32] |= 1 << (k % 32);
                    let (at, zoom) = square(&cast.window[4 * k..4 * k + 4], k);
                    nodes.push(at, zoom);
                }
            }
        }
        // The sun's cast reads the world as the eye's rays do, from the
        // eye's place at its zooms, but not its look: a turn of the eye
        // alone casts it no ray again. Its clipmap keeps what it holds while
        // the light's frame and the world under it read as they did
        // (`sun_read`); then the eye's place, quality or zooms or the box
        // moved (`sun_at`), or the nodes under it (`nodes`), casts only the
        // texels the box moved onto, those whose stop the eye now asks at
        // another zoom and those whose ray passes over a node moved, each
        // judged where it lies (pxl-spec-core `sun`). None where no shadow is
        // cast.
        let shadows = cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS].iter().any(|&w| w != 0);
        let r = &mut c.sun_read;
        r.begin();
        r.read(&[sun_map.1, u32::from(shadows)]);
        r.mark(shards_moved || shards_grow || overwrites_moved || nodes.whole);
        let whole = !r.end();
        let a = &mut c.sun_at;
        a.begin();
        a.read(&cast.eye[eye::ORIGIN..eye::STEPS]);
        a.read(&cast.eye[eye::TAU..eye::TAU + 1]);
        a.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
        a.read(&cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS]);
        let moved = !a.end();
        c.sun_cast = shadows && (whole || moved || nodes.len != 0);
        if c.sun_cast {
            // What the map holds, written over the kept corner's words, which
            // keep nothing as the box is aimed (`Sun::of`).
            if let (false, Some(kept)) = (whole, c.sun_kept) {
                for (k, w) in sun::KEPT.into_iter().zip(kept) {
                    cast.eye[eye::SUN + k] = w as u32;
                }
            }
            c.sun_kept = Some(sun::CORNER.map(|k| cast.eye[eye::SUN + k] as i32));
            let len = if whole { 0 } else { nodes.len };
            nodes.words[0] = len as u32;
            g.write_buffer(c.sun_moved, 0, bytes(&nodes.words[..4 + 4 * len]));
            g.write_buffer(c.sun_list, 0, bytes(&sun::LIST_EMPTY));
        }
        if eye_moved || c.sun_cast {
            g.write_buffer(c.eye, 0, bytes(&cast.eye));
        }
        if self.kind == FrameKind::cast {
            c.forgets = if c.forget {
                observed::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) {
                observed::FORGETS_MOVED
            } else {
                observed::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which slots; forgotten whole, no tile taken and its first frame,
            // else the next frame.
            let mut head = [0u32; observed::HEAD_WORDS as usize];
            c.frame = if c.forgets == observed::FORGETS_ALL { 1 } else { c.frame + 1 };
            head[observed::FRAME as usize] = c.frame;
            head[observed::FORGETS as usize] = c.forgets;
            head[observed::MOVED_AT as usize..].copy_from_slice(&c.moved);
            let from = if c.forgets == observed::FORGETS_ALL { 0 } else { observed::FRAME as usize };
            g.write_buffer(c.seen, word_bytes(from), bytes(&head[from..]));
            (c.forget, c.moved) = (false, [0; observed::MOVED_WORDS as usize]);
        }
        let bound = [c.eye, c.window, ground, rays.0, looks, c.shards, c.overwrites, pool, c.beams, c.seen];
        if bound != c.bound {
            release(c.group);
            c.group = cast_group(g, &c.built, bound);
            c.bound = bound;
        }
        let sun_bound = [c.eye, c.window, ground, c.shards, c.overwrites, pool, sun_map.0];
        if sun_bound != c.sun_bound {
            for group in c.sun_groups {
                release(group);
            }
            c.sun_groups = sun_groups(g, [&c.sun_mark_built, &c.sun_built], sun_bound, c.sun_moved, c.sun_list);
            c.sun_bound = sun_bound;
        }
        let fresh_squares = self.ground.as_ref().map_or(0, |gb| gb.fresh);
        let world = [c.eye, c.window, ground, c.shards, c.overwrites, pool, fresh_squares];
        let grows = Grows {
            window: cast.window,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS].try_into().or_bug(Errno::ENOTRECOVERABLE),
            filled: fresh,
            eye: &cast.eye,
        };
        c.grown.prepare(g, cast.decor, grows, meshes, world);
        let eye = c.eye;
        self.regroup(g, [rays.0, eye]);
    }

    /// The columns of the frames this turn uploaded, their tops from their
    /// samples, then their ways' runs, then their levels a dispatch, level 0
    /// a column's own and each over the one before, and cells and bounds read back where the CPU looks them
    /// up: ahead of the frame, as they outlive it, so no frame the window
    /// names is cast before its cells are written, even when the frame
    /// proper is dropped.
    /// Whether it recorded any.
    fn grids(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> bool {
        let Some(c) = self.ground.as_mut().filter(|c| c.fresh_len != 0) else { return false };
        let pass = g.compute_pass(encoder, stamp.pass());
        // A re-filled square's old ways' blocks given back, off its root word
        // before its build writes another (pxl-spec-core `ways::tree`).
        g.set_pipeline(pass, c.free_built.pipeline);
        g.set_bind_group(pass, 0, c.free_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.heights_built.pipeline);
        g.set_bind_group(pass, 0, c.heights_group, &[]);
        let w = compute::HEIGHTS_WORKGROUP;
        g.dispatch(pass, SQUARE.div_ceil(w[0]), SQUARE.div_ceil(w[1]), c.fresh_len);
        // A column a thread, over the tops just drawn: the columns' runs
        // counted, summed a square a workgroup (pxl-spec-core
        // `fill::SCAN_THREADS`), written, before the levels fold them.
        g.set_pipeline(pass, c.ways_built.pipeline);
        for k in 0..3 {
            g.set_bind_group(pass, 0, c.ways_group, &[k * SLOT_STRIDE]);
            let across = if k == 1 { 1 } else { SQUARE_CELLS / compute::WAYS_WORKGROUP[0] };
            g.dispatch(pass, across, 1, c.fresh_len);
        }
        g.set_pipeline(pass, c.bound_built.pipeline);
        for j in 0..=LEVELS {
            g.set_bind_group(pass, 0, c.bound_group, &[j * SLOT_STRIDE]);
            let side = (SQUARE >> j).div_ceil(compute::BOUND_WORKGROUP[0]);
            g.dispatch(pass, side, side, c.fresh_len);
        }
        // Each square's ways as blocks, a workgroup a square, over its runs
        // and the levels just folded; then what its build gave back pushed,
        // a dispatch after, so no push lands under another square's ids
        // while it reads them.
        g.set_pipeline(pass, c.tree_built.pipeline);
        g.set_bind_group(pass, 0, c.tree_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.freed_built.pipeline);
        g.set_bind_group(pass, 0, c.freed_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.end(pass);
        true
    }

    /// The squares the grids wrote mapped back where the CPU looks them up,
    /// once the encoder that wrote them is submitted: a square's columns,
    /// levels and runs lie together (`window::BACK_WORDS`), so one mapping
    /// brings them all.
    ///
    /// A square asked takes a free buffer of `READS`, which maps once at a
    /// time and lands whatever fill it carried. One that landed for the
    /// square's latest fill is told `landed`. One that landed for an earlier
    /// fill is discarded, and the latest fill copied again: its bytes went
    /// into the square's cells while the kernel holds the square on its way
    /// (or absent, if it moved), which nothing reads, and the latest fill's
    /// landing writes them again before it is told. A landed buffer is free again; a square with none
    /// free waits, Sent, for a later turn. A mapping still on its way is
    /// waited for, looked at only when a turn runs: nothing asks a turn for
    /// it. The turn's squares are done with here.
    ///
    /// On a device, a mapping that completes after the last asked turn is
    /// seen at the next turn: the host learns of it only by polling the
    /// device, and a thread waiting there (wgpu `poll(Wait)`) holds the lock a
    /// surface's present takes (wgpu-core's `Device::poll` reads its
    /// `snatchable_lock` across the fence wait, `Surface::present` writes
    /// it), so none waits. Where read-backs matter on a settled
    /// view, the platform's own completion (a command buffer's handler, a
    /// timeline semaphore) is the wake.
    fn read_back(&mut self, g: &Gpu, ground: &mut GroundSquares) {
        let (landed, joined) = (&mut *ground.landed, ground.joined);
        let most = [ground.pool[1].min(ways::tree::POOL_BLOCKS), ground.scratch[1].min(ways::tree::SQUARE_BLOCKS)];
        // The fold, once its mapping is free: a frame-old copy reads safely,
        // as an entry answers only for the block whose key it holds.
        if let Some(c) = self.cast.as_mut().filter(|c| c.joined_back && *c.joined_mapping & DONE != 0) {
            let (e, size) = (g.encoder(), word_bytes(window::EYE_JOINED_WORDS as usize));
            g.copy_buffer(e, (c.window, word_bytes((4 * window::JOINED_AT) as usize)), (c.joined_read, 0), size);
            g.submit(e);
            g.map_read(c.joined_read, 0, size, joined, &mut c.joined_mapping, 1);
            c.joined_back = false;
        }
        let Some(c) = self.ground.as_mut() else { return };
        // A read lands with its square's words and its records both: told
        // where it is of the square's latest fill, discarded where a later
        // fill asked again, whose own read brings it. A build with no
        // record ran out of the pool or the scratch it was built in, its
        // count saying which (`short_of`), which grows ahead of the next
        // turn's squares where it is as built still and may (`ran_out`,
        // `grow`, `grow_scratch`), the square filled again; one past its
        // own most is refused at once; one whose records outgrew its read is
        // filled again, every read's room grown past them.
        for k in 0..READS {
            let (s, ticket) = c.held[k];
            if s == FREE || c.mapping[k] & DONE == 0 || c.record_mapping[k] & DONE == 0 {
                continue;
            }
            c.held[k] = (FREE, 0);
            if c.asked[s as usize].ticket != ticket {
                continue;
            }
            c.asked[s as usize].ticket = 0;
            let room = &c.record_rooms[k];
            let n = room.first().copied().unwrap_or(0);
            let words = 1 + n.min(ways::tree::SQUARE_BLOCKS) * ways::tree::RECORD_WORDS;
            let landing = if n == ways::tree::NONE {
                Landing::Refused
            } else if let Some(short) = short_of(n) {
                let (i, now) = (short as usize, [c.blocks, c.scratch_blocks]);
                match ran_out(c.built_in[k][i], now[i], most[i]) {
                    RanOut::Again => Landing::Again,
                    RanOut::Grow => {
                        c.ran_out[i] = true;
                        Landing::Again
                    }
                    RanOut::Refused => Landing::Refused,
                }
            } else if words as usize > room.len() {
                c.record_bytes = c.record_bytes.max(word_bytes(words as usize).next_power_of_two().min(ways::tree::RECORDS_BYTES));
                Landing::Again
            } else {
                Landing::Built(&room[..words as usize])
            };
            landed(s, ticket, landing);
        }
        // Each square the frame handed, its words and its build's records
        // copied in its frame, into a read `hand` kept free for it: as much
        // of its records as the read's room holds, and its scratch's do.
        let fresh = core::mem::take(&mut c.fresh_len);
        if fresh == 0 {
            return;
        }
        let e = g.encoder();
        let mut taken = [false; READS];
        let mut free = 0;
        for h in 0..fresh {
            let s = c.handed[h as usize];
            let Some(k) = (free..READS).find(|&k| c.held[k].0 == FREE) else { break };
            free = k + 1;
            c.room(g, k);
            g.copy_buffer(e, (c.ground, s * SQUARE_BYTES + COLUMNS_AT * 4), (c.reads[k], 0), BACK_WORDS * 4);
            let sb = c.scratch_blocks;
            let records = (h * ways::tree::scratch_words(sb) + ways::tree::records_at(sb)) as usize;
            let words = c.record_rooms[k].len().min((1 + sb * ways::tree::RECORD_WORDS) as usize);
            g.copy_buffer(e, (c.scratch, word_bytes(records)), (c.record_reads[k], 0), word_bytes(words));
            (c.held[k], c.built_in[k], taken[k]) = ((s, c.asked[s as usize].ticket), [c.blocks, sb], true);
        }
        g.submit(e);
        for k in (0..READS).filter(|&k| taken[k]) {
            let (s, ticket) = c.held[k];
            g.map_read(c.reads[k], 0, BACK_WORDS * 4, c.asked[s as usize].cells, &mut c.mapping[k], ticket);
            let room = &mut c.record_rooms[k];
            g.map_read(c.record_reads[k], 0, word_bytes(room.len()), off_of(room.as_ptr()) as u32, &mut c.record_mapping[k], ticket);
        }
    }

    /// The eye's windows folded over their squares where their slots moved
    /// (`joined_cs`): ahead of the frame, as the fold outlives it, and then
    /// mapped back (`read_back`).
    fn fold(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> bool {
        let Some(c) = self.cast.as_mut().filter(|c| c.refold && c.group != 0) else { return false };
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, c.group, &[]);
        g.set_pipeline(pass, c.joined_built.pipeline);
        // A level a dispatch, each over the one under it, its level the
        // dispatch's depth (`joined_cs`).
        for l in 1..=window::JOINED_LEVELS {
            let blocks = (window::ACROSS >> l) * (window::ACROSS >> l);
            g.dispatch(pass, (blocks * window::EYE_WINDOWS).div_ceil(compute::JOINED_WORKGROUP[0]), 1, l);
        }
        g.end(pass);
        (c.refold, c.joined_back) = (false, true);
        true
    }

    /// The cast where the frame casts, the world's depth from the sun with
    /// it where `sun`, then the decor grown over the world where it moves:
    /// each a pass of its own over the cast's group, timed in its own span,
    /// so what each costs is read apart; the sun's in the shadow's, with
    /// the map's pass, which is not drawn at the world's pixels
    /// (`PER_PIXEL`).
    fn dispatch(&self, g: &Gpu, encoder: u32, stamps: &mut [Stamp]) {
        let Some(c) = self.cast.as_ref() else { return };
        if self.kind == FrameKind::cast {
            // The observed pyramid forgotten first where it is; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start; the pixels
            // off the samples about them, or reading it from their cell's
            // start, marking the pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once. One pass, a dispatch each, each
            // reading what the ones before wrote (pxl-spec-core `observed`).
            let pass = g.compute_pass(encoder, stamps[SPAN_CAST].pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forgets != observed::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_built.pipeline);
                g.dispatch(pass, observed::ENTRIES.div_ceil(compute::FORGET_WORKGROUP[0]), 1, 1);
            }
            for (built, [x, y], w) in [
                (&c.beam_built, c.grid.corners, compute::BEAM_WORKGROUP),
                (&c.cells_built, c.grid.span, compute::CELLS_WORKGROUP),
                (&c.sample_built, c.grid.samples, compute::SAMPLE_WORKGROUP),
                (&c.built, c.size, compute::CAST_WORKGROUP),
                (&c.miss_built, c.size, compute::MISS_WORKGROUP),
            ] {
                g.set_pipeline(pass, built.pipeline);
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            }
            g.end(pass);
        }
        if c.sun_cast {
            // The tiles a slot of which walks again listed, a thread a tile;
            // then the walks, a workgroup a tile listed (pxl-spec-core
            // `sun::TILE`), so a slot that stands holds no lane of a walk's.
            let pass = g.compute_pass(encoder, stamps[SPAN_SHADOW].pass());
            g.set_bind_group(pass, 0, c.sun_groups[0], &[]);
            g.set_pipeline(pass, c.sun_mark_built.pipeline);
            let tiles = (shadow::SIZE / sun::TILE).pow(2);
            g.dispatch(pass, tiles.div_ceil(compute::SUN_MARK_WORKGROUP[0]), 1, 1);
            g.set_bind_group(pass, 0, c.sun_groups[1], &[]);
            g.set_pipeline(pass, c.sun_built.pipeline);
            g.dispatch_indirect(pass, c.sun_list, 0);
            g.end(pass);
        }
        if self.grows() {
            let pass = g.compute_pass(encoder, stamps[SPAN_GROW].pass());
            c.grown.dispatch(g, pass);
            g.end(pass);
        }
    }

    /// Whether the world's depth from the sun is cast again this frame.
    fn sun_casts(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.sun_cast)
    }

    /// Whether the decor grown over the cast's world moves this frame.
    fn grows(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.grown.works())
    }

    /// The decor the GPU grew this frame, as the draws read it.
    fn grown(&self) -> Option<Drawn> {
        self.cast.as_ref().and_then(|c| c.grown.drawn())
    }

    /// Returns the group 0 it leaves bound: none where no eye casts.
    fn draw(&self, g: &Gpu, pass: u32) -> u32 {
        self.draw_by(g, pass, &self.built)
    }

    /// Its depth alone, the occlusion's prepass's (gpu/ao.rs).
    fn draw_depth(&self, g: &Gpu, pass: u32) -> u32 {
        self.draw_by(g, pass, &self.depth)
    }

    fn draw_by(&self, g: &Gpu, pass: u32, built: &Built) -> u32 {
        if self.kind == FrameKind::plain || self.group == 0 {
            return 0;
        }
        g.set_pipeline(pass, built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.group
    }
}

/// The cast's group 0 over `bound`: the eye, the window and its fold
/// (`joined_cs` writes it, the cast reads it), the tiles' frames, the
/// surface's rays, the squares' looks, the service area's index, the
/// overwrites, the ways' pool, the beam rays' lines and the cells' starts,
/// and the observed pyramid.
fn cast_group(g: &Gpu, built: &Built, bound: [u32; 10]) -> u32 {
    let [eye, window, ground, rays, looks, shards, overwrites, pool, beams, seen] = bound;
    let entries = [
        buffer_entry(0, eye),
        buffer_entry(1, window),
        buffer_entry(2, ground),
        view_entry(3, rays),
        buffer_entry(4, looks),
        buffer_entry(5, shards),
        buffer_entry(6, overwrites),
        buffer_entry(7, pool),
        buffer_entry(8, beams),
        buffer_entry(9, seen),
    ];
    g.bind_group(built.layouts[0], &entries)
}

/// The sun's mark's group 0 and its cast's, over `bound`: the eye, the
/// window, the tiles' frames, the service area's index, the overwrites and
/// the ways' pool, the cast's alone; the clipmap; the nodes moved under it
/// (`moved`) and the tiles listed (`list`).
fn sun_groups(g: &Gpu, [mark, walk]: [&Built; 2], bound: [u32; 7], moved: u32, list: u32) -> [u32; 2] {
    let [eye, window, ground, shards, overwrites, pool, sun] = bound;
    let entries = [
        (0, buffer_entry(0, eye)),
        (1, buffer_entry(1, window)),
        (2, buffer_entry(2, ground)),
        (5, buffer_entry(5, shards)),
        (6, buffer_entry(6, overwrites)),
        (7, buffer_entry(7, pool)),
        (10, view_entry(10, sun)),
        (11, buffer_entry(11, moved)),
        (12, buffer_entry(12, list)),
    ];
    // Each the entries its layout declares (build.rs `compute`): the mark
    // reads the clipmap and the nodes moved alone.
    let of = |declared: &[u32]| entries.iter().filter(|(b, _)| declared.chunks(3).any(|d| d[0] == *b)).map(|&(_, e)| e).collect::<Vec<_>>();
    [g.bind_group(mark.layouts[0], &of(&compute::SUN_MARK_GROUP)), g.bind_group(walk.layouts[0], &of(&compute::SUN_GROUP))]
}

/// A variant picker case: its own frame, drawn at one sample with no shadow:
/// a blank map bound, and no strength to read it by.
struct Case {
    frame_buffer: u32,
    frames: FrameGroups,
    skinned: SkinnedPass,
    /// What its picture is made of as last drawn: its frame words, its
    /// actor's draw and uniform, its skin and its surface; it stands, and
    /// its surface keeps its picture, while they do.
    read: Read,
}

const CASE_EYE: V3 = [0.0, -2.2, 0.9];
const CASE_TARGET: V3 = [0.0, 0.0, 0.55];
const CASE_FOV_DEG: f64 = 35.0;
/// The case's near plane: a room of its own, a model a unit tall in it.
const CASE_NEAR: f64 = 0.1;

impl Case {
    fn new(g: &Gpu, pipelines: &mut Pipelines, shadow: &Shadow, ao: &ScreenAo, air: [Entry; AIR_ENTRIES], align: u32) -> Case {
        let frame_buffer = g.buffer(word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let layout = frame_layout(g, pipelines);
        Case {
            frame_buffer,
            frames: FrameGroups::new(g, layout, frame_buffer, shadow.blank_entries(1), [ao.blank_entry(7); 2], air),
            skinned: SkinnedPass::new(g, pipelines, align),
            read: Read::default(),
        }
    }

    fn release(self) {
        self.frames.release();
        release(self.frame_buffer);
        self.skinned.release();
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
    layers: Layers,
    moon: ImageMap,
    decor: DecorPass,
    impostor: ImpostorPass,
    skinned: SkinnedPass,
    puck: PuckPass,
    badges: BadgePass,
    debug: DebugPass,
    screen: RaysPass,
    /// The frame's whole input as the last frame drew it (stage 2): the
    /// frame's, the sky's and the stars' words, the air's, the surface's
    /// textures, the draws and the blocks they read, the badges' sheet, the
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
        let frame_buffer = g.buffer(word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let sky_buffer = g.buffer(word_bytes(SKY_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buffer);
        let atmosphere = Atmosphere::new(g, &mut pipelines, frame_buffer, sky_buffer);
        let air = atmosphere.entries();
        let stars = StarPass::new(g, &mut pipelines, frame_buffer, air);
        let clouds = CloudPass::new(g, &mut pipelines, frame_buffer, sky_buffer, air);
        let mips = Mips::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let layers = Layers::new(g);
        let sky_built = pipelines.build(g, PIPELINE_SKY, g.canvas());
        let moon = ImageMap::new(g, MOON_TEXELS, MOON_TEXELS);
        let mut sky_entries = vec![buffer_entry(0, frame_buffer), buffer_entry(19, sky_buffer), view_entry(2, moon.view), sampler_entry(3, moon.sampler)];
        sky_entries.extend(air);
        let sky_group = g.bind_group(sky_built.layouts[0], &sky_entries);
        let draws_buffer = g.buffer(DRAWS_INITIAL * 4, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        let shadow = Shadow::new(g, &mut pipelines);
        let models = FrameGroups::new(g, frame_layout(g, &mut pipelines), frame_buffer, shadow.entries(1), [ao.blank_entry(7); 2], air);
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
            badges: BadgePass::new(g, &mut pipelines, frame_buffer, device.align),
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
            layers,
            moon,
            cases: [const { None }; SURFACES],
            order: Vec::new(),
        };
        r.box_groups(g);
        r
    }

    /// What a block's face is painted by, wherever it is drawn from
    /// (paint.wgsl): the frame, the sun's shadow, the layers, the occlusion
    /// and the air.
    fn lit_entries(&self, frame: u32) -> Vec<Entry> {
        let [map, sampler] = self.shadow.entries(1);
        let mut e = vec![
            buffer_entry(0, frame),
            map,
            sampler,
            view_entry(4, self.layers.view),
            buffer_entry(5, self.layers.table),
            sampler_entry(6, self.layers.sampler),
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
        self.models.rebind(g, self.frame_buffer, [self.ao.entry(7), self.ao.blank_entry(7)]);
    }

    /// `reach` how far the eye observes, in units; none without an eye.
    fn lay_out(&mut self, g: &Gpu, width: u32, height: u32, env: &[f32], reach: f32) {
        let s = stage_lane();
        let (mut m, mut proj) = (mat4::new(), mat4::new());
        proj.copy_from_slice(&s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16]);
        // The badges measure their size in view space.
        let b = &mut self.badges;
        put_mat(&mut b.view, 0, s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].try_into().or_bug(Errno::ENOTRECOVERABLE));
        put_mat(&mut b.proj, 0, &proj);
        b.px_k = (s[STAGE_AT::VIEWPORT + 1] / (2.0 * libm::tan(s[STAGE_AT::FOV] * core::f64::consts::PI / 360.0))) as f32;

        let u = &mut self.frame;
        m.copy_from_slice(&s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16]);
        put_mat(u, VIEW_PROJ, &m);
        // Everything is drawn in the eye's frame; positions are taken against the eye
        // in f64 before they are narrowed.
        let eye_world = [s[STAGE_AT::EYE], s[STAGE_AT::EYE + 1], s[STAGE_AT::EYE + 2]];
        let eye = [0.0; 3];
        put_ray(&mut self.sky, sky::RAY, &m, &eye);
        self.clouds.lay_out([eye_world[0], eye_world[1]], env, s[STAGE_AT::CLOUDS], reach * env[ENV_AIR + 1]);
        self.stars.lay_out(&m, env, width);
        self.sky[sky::POLE..sky::POLE + 3].copy_from_slice(&env[ENV_STARS + 8..ENV_STARS + 11]);
        self.sky[sky::MAPPED] = if self.moon.loaded { 1.0 } else { 0.0 };
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
        }
        u[EDGE_STRENGTH] = s[STAGE_AT::EDGE_STRENGTH] as f32;
        u[CELL] = s[STAGE_AT::LOD_CELL] as f32;
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
        u[MAP..MAP + 3].copy_from_slice(&[lon as f32, (eye_world[1] / MERCATOR_RADIUS_U) as f32, MERCATOR_RADIUS_U as f32]);
        // The sun's map, as wide as the casters reach; none where the
        // shadows are off.
        let far = s[STAGE_AT::SHADOWS];
        self.shadows = far > 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        if self.shadows {
            // Up to a power of two, so the snap grid and the texel move only in steps;
            // the shadows fade by the casters' own reach.
            let half = libm::exp2(libm::ceil(libm::log2(far)));
            self.shadow.aim(&env[ENV_SUN..ENV_SUN + 3], eye, half, eye_world.map(|v| -v));
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
        let b = blocks();
        let mut bound = 0;
        for d in draws.of(PIPELINE_GHOST_BOX) {
            let (handle, box_cnt, first_box, uniform) = (d[0], d[1], d[2], d[3]);
            let buffer = b.get(handle).map(|(buffer, ..)| buffer);
            // A missing buffer is a fault in the list. Skipping it silently once drew no
            // ground at all with every gauge reading normal.
            let Some(buffer) = buffer else { xpute_kit::bug!(ENOTRECOVERABLE, handle) };
            if bound == 0 {
                g.set_pipeline(pass, self.ghost.pipeline);
                g.set_bind_group(pass, 0, self.bind_group, &[]);
            }
            if buffer != bound {
                g.set_vertex_buffer(pass, 0, buffer, 0, 0);
                g.set_vertex_buffer(pass, 1, buffer, 0, 0);
                bound = buffer;
            }
            g.draw(pass, BOX_VERTICES, box_cnt, uniform * BOX_VERTICES, first_box);
        }
    }

    /// The world image's opaque models: the decor grown over the world. The
    /// impostors are the lit pass's alone. `bound` comes back as the group 0
    /// the pass has.
    fn draw_opaque(&mut self, g: &Gpu, pass: u32, b: &mut Blocks, stage: Stage) -> u32 {
        let mut bound = 0;
        self.decor.draw_grown(g, pass, self.screen.grown(), b, stage, (self.models.frame, &mut bound));
        bound
    }

    /// A frame in two stages, each standing while what it reads stands:
    /// the cast's rays (stage 1, `RaysPass::prepare`); and the frame,
    /// everything the world and its light make off those rays, then over it
    /// what moves on its own — the actors, whose clips play, the clouds,
    /// which the wind carries, the puck, the ghosts, the badges, the gizmos
    /// (stage 2, `frame_read`). The rays hold the world as the cast left
    /// it, so the frame draws it again off them rather than off an image of
    /// it: that costs what laying an image would.
    fn frame(&mut self, g: &Gpu, world: &mut Surface, mut input: FrameInput) -> (FrameKind, FrameStat) {
        let mut read = core::mem::take(&mut self.frame_read);
        read.begin();
        self.decor.set_models(input.decor);
        self.impostor.set_models(input.decor.iter().map(|&(mesh, image, extent)| (mesh, image.map(|i| i.0), extent)));
        self.skinned.set_skins(input.skins);
        read.read(input.skins);
        read.mark(self.layers.step(g, &self.mips, input.pack));
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
        read.mark(self.atmosphere.set_earth_tiles(g, tiles.window, tiles.layers, tiles.upload));
        read.mark(self.badges.sync_sheet(g, &self.mips, input.billboards, input.text));
        let (records, uniforms, blocks_moved) = take_lists(Some(g), input.lists);
        read.mark(blocks_moved);
        // Each list's writers write again only where it moved.
        let uniforms_moved = read.read(uniforms);
        let lists_moved = read.read(records) | uniforms_moved;
        read.read(input.cases);

        targets(g, world);
        // How far the eye observes, as far as the clouds are drawn.
        let reach = cast.as_ref().map_or(0.0, |c| f32::from_bits(c.eye[eye::REACH]));
        self.lay_out(g, world.width, world.height, input.environment, reach);
        // The light box the world is cast from, as the map is aimed; none
        // where no shadow is cast.
        if let Some(c) = cast.as_mut() {
            let sun = if self.shadows { self.shadow.sun } else { [0; eye::SUN_WORDS] };
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
            self.draws_buffer = g.buffer(self.draws_bytes, buffer_usage::STORAGE | buffer_usage::COPY_DST);
            self.box_groups(g);
        }
        if !uniforms.is_empty() && (grew || uniforms_moved) {
            // Read where the draw list holds them, one record however many.
            g.upload_buffer(self.draws_buffer, 0, bytes(uniforms));
        }
        let mut order = core::mem::take(&mut self.order);
        let draws = Draws::sort(records.as_chunks::<DRAW_WORDS>().0, &mut order);
        // The camera gizmo listed tints the rays' blocks (screen_lit.wgsl `cell_share`).
        self.frame[GIZMO] = if draws.of(PIPELINE_CAMERA_GRID).len() > 0 { 1.0 } else { 0.0 };
        // Each uniform written where the image's read finds it moved.
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
        let b = blocks();
        self.decor.prepare(g, &draws, uniforms, lists_moved, b);
        self.impostor.prepare(g, draws.of(PIPELINE_IMPOSTOR), uniforms, lists_moved, b);
        self.skinned.prepare(g, draws.of(PIPELINE_SKINNED), uniforms, lists_moved, b);
        self.puck.prepare(g, &draws, uniforms, lists_moved, b);
        self.badges.prepare(g, world.depth_view, (world.width, world.height), &draws, uniforms, b);
        self.debug.prepare(g, &draws, uniforms, lists_moved);
        let meshes: [u32; GROUPS as usize] = core::array::from_fn(|k| self.decor.model_of(g, k as u32, b).map_or(0, |m| m.2));
        self.screen
            .prepare(g, cast, &meshes, (world.rays_view, [world.width, world.height]), (self.shadow.world, self.shadow.frame));
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
        let aired = self.atmosphere.froxel(g, ahead, [&self.frame, &self.sky], exposed, &mut stamps[SPAN_AIR]);
        read.mark(aired);
        if baked || gridded || folded || exposed || aired {
            g.submit(ahead);
        } else {
            release(ahead);
        }
        self.atmosphere.read_back(g);
        self.screen.read_back(g, &mut input.ground);
        if read.read(self.clouds.words()) {
            self.clouds.write(g);
        }
        // The frame stands where all it draws from read as last time over
        // rays that stood: nothing is drawn, nothing submitted, and the
        // canvases keep the frame they show.
        if read.end() && self.screen.kind != FrameKind::cast && !self.screen.grows() && !self.screen.sun_casts() && !gridded {
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
            let pass = self.shadow.begin(g, encoder, self.screen.kind != FrameKind::plain, &mut stamps[SPAN_SHADOW]);
            let mut bound = 0;
            let light = self.shadow.light_group;
            self.decor.draw_grown(g, pass, self.screen.grown(), b, Stage::Depth, (light, &mut bound));
            self.decor.draw(g, pass, &draws, b, Stage::Depth, (light, &mut bound));
            self.skinned.draw(g, pass, draws.of(PIPELINE_SKINNED), b, Stage::Depth, (light, &mut bound));
            g.end(pass);
        }
        let a = &mut self.ao_read;
        a.begin();
        a.read(&self.frame);
        a.read(self.ao.words());
        a.read(&[world.width, world.height, u32::from(self.screen_ao), u32::from(self.screen.grown().is_some())]);
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
        let pass = g.pass(encoder, &pass_desc(canvas_view, Load::Clear, world.depth_view, Load::Clear, 0.0, stamps[SPAN_WORLD].pass()));
        self.draw_opaque(g, pass, b, Stage::Lit);
        let mut bound = self.screen.draw(g, pass);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor.draw_grown(g, pass, self.screen.grown(), GROUPS, (self.models.frame, &mut bound));
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
        self.skinned.draw(g, pass, draws.of(PIPELINE_SKINNED), b, Stage::Lit, (over.0, &mut *over.1));
        self.decor.draw(g, pass, &draws, b, Stage::Lit, (over.0, &mut *over.1));
        self.impostor.draw(g, pass, draws.of(PIPELINE_IMPOSTOR), b, over);
        self.clouds.draw(g, pass);
        self.puck.draw_beacon(g, pass, &draws);
        self.puck.draw_zone(g, pass, &draws, b);
        self.draw_boxes(g, pass, &draws);
        let mut bound = 0;
        self.decor.draw_ghosts(g, pass, &draws, b, (self.models.frame, &mut bound));
        g.end(pass);
        self.badges.draw(g, encoder, canvas_view, world.depth_view, &draws, b, &mut stamps[SPAN_BADGES]);
        self.debug.draw(g, encoder, canvas_view, world.depth_view, &draws, b, &mut stamps[SPAN_DEBUG]);
        self.draw_cases(g, encoder, Self::cases(&draws, input.cases), uniforms, blocks_moved || tables || exposed, &mut stamps[SPAN_CASES]);
        let written = stamps.iter().enumerate().filter(|(_, s)| s.written()).fold(0, |m, (k, _)| m | 1 << k);
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
            // Its cast and its image were dropped with it: the rays and the
            // image hold older ones.
            if let Some(c) = self.screen.cast.as_mut() {
                c.read.forget();
                c.sun_read.forget();
                c.grown.dropped();
            }
            for case in self.cases.iter_mut().flatten() {
                case.read.forget();
            }
            read.forget();
            self.ao_read.forget();
        } else if self.screen.grows() {
            // Drawn: what its decor marked by is done with.
            if let Some(c) = self.screen.cast.as_mut() {
                c.grown.drawn_frame();
            }
        }
        read.end();
        self.frame_read = read;
        let kind = if skipped { FrameKind::dropped } else { self.screen.kind };
        let mut stat = self.stat(draws.records, world, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
        stat.gpu_px = gpu_px;
        self.order = order;
        (kind, stat)
    }

    /// The cases drawn this frame: a surface still a case, and its actor's draw.
    fn cases<'a, 'd>(draws: &'a Draws<'d>, cases: &'a [(u32, u32, Image)]) -> impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])> + 'a {
        cases.iter().filter_map(move |&(id, model, image)| {
            surface(id).filter(|s| s.role == surface_role::CASE)?;
            let own = draws.of(PIPELINE_SKINNED).find(|d| d[2] == model)?;
            Some((id, model, image, own))
        })
    }

    /// Each case's targets, passes and skin, made here so drawing makes nothing
    /// that outlives its frame.
    fn prepare_cases(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], cases: &[(u32, u32, Image)], moved: bool) {
        for (id, model, image, own) in Self::cases(draws, cases) {
            let Some(s) = surface(id) else { continue };
            targets(g, s);
            let slot = handle_slot(id) as usize;
            if self.cases[slot].is_none() {
                self.cases[slot] = Some(Case::new(g, &mut self.pipelines, &self.shadow, &self.ao, self.atmosphere.entries(), self.align));
            }
            let case = self.cases[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            case.skinned.set_skins(&[(model, image)]);
            case.skinned.prepare(g, core::iter::once(own), uniforms, moved, blocks());
        }
    }

    /// Each case whose picture's makings moved, `moved` what moved without
    /// words: the models' blocks, the air's tables or its exposure.
    fn draw_cases<'d>(&mut self, g: &Gpu, encoder: u32, cases: impl Iterator<Item = (u32, u32, Image, &'d [u32; DRAW_WORDS])>, uniforms: &[f32], moved: bool, stamp: &mut Stamp) {
        for (id, model, image, own) in cases {
            let Some(s) = surface(id) else { continue };
            let Some(case) = self.cases[handle_slot(id) as usize].as_mut() else { continue };
            let u = uniform_of(uniforms, own);
            let actor = [u[0] as f64, u[1] as f64, u[2] as f64];
            let mut frame = self.frame;
            let u = &mut frame;
            let (mut proj, mut view, mut t, mut m, mut n) = (mat4::new(), mat4::new(), mat4::new(), mat4::new(), mat4::new());
            mat4::perspective(&mut proj, CASE_FOV_DEG.to_radians(), s.width as f64 / s.height as f64, CASE_NEAR);
            mat4::look_at(&mut view, CASE_EYE, CASE_TARGET, [0.0, 0.0, 1.0]);
            mat4::translation(&mut t, -actor[0], -actor[1], -actor[2]);
            mat4::mul(&mut n, &proj, &view);
            mat4::mul(&mut m, &n, &t);
            put_mat(u, VIEW_PROJ, &m);
            u[CAMERA_POS] = (CASE_EYE[0] + actor[0]) as f32;
            u[CAMERA_POS + 1] = (CASE_EYE[1] + actor[1]) as f32;
            u[CAMERA_POS + 2] = (CASE_EYE[2] + actor[2]) as f32;
            u[VIEWPORT] = s.width as f32;
            u[VIEWPORT + 1] = s.height as f32;
            u[VIGNETTE] = 0.0;
            u[SCREEN_AO] = 0.0;
            u[SHADOW_STRENGTH] = 0.0;
            let r = &mut case.read;
            r.begin();
            r.read(&frame);
            r.read(own);
            r.read(uniform_of(uniforms, own));
            r.read(&[model, image.0, s.width, s.height, s.depth]);
            r.mark(moved);
            if r.end() {
                continue;
            }
            g.write_buffer(case.frame_buffer, 0, bytes(&frame));
            let view_id = g.current(s.handle);
            let pass = g.pass(encoder, &pass_desc(view_id, Load::Clear, s.depth_view, Load::Clear, 0.0, stamp.pass()));
            case.skinned.draw(g, pass, core::iter::once(own), blocks(), Stage::Lit, (case.frames.frame, &mut 0));
            g.end(pass);
            release(view_id);
        }
    }

    /// What the scheduler asked for: a draw passed over because its block, skin or
    /// sprite is not on the device yet is still counted. The draws are counted
    /// only while `counted`, the profiler's, so a frame no one reads counts none.
    fn stat(&self, draws: &[[u32; DRAW_WORDS]], world: &Surface, counted: bool, gpu_spans: [f64; GPU_SPAN_NAMES]) -> FrameStat {
        let b = blocks();
        let mut census: DrawCensus = [[0.0; 2]; DRAW_PIPELINES.len()];
        for d in draws.iter().filter(|_| counted) {
            let tris = match d[4] {
                PIPELINE_GHOST_BOX => d[1] * 12,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, index_cnt)| index_cnt / 3),
                PIPELINE_PUCK_ZONE => 12 * d[1],
                PIPELINE_PUCK_BEACON => 8,
                PIPELINE_BADGE => 2 * d[1],
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
            blocks: b.count as f64,
            block_bytes: b.bytes as f64,
            // The depth targets at the world's size: its own, one f32 a pixel, AO's and the badges' regions'.
            depth_bytes: px * 4.0 + self.ao.depth_bytes() + self.badges.depth_bytes(),
            layers_bytes: self.layers.bytes as f64,
            census,
            gpu_spans,
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
    let Some(d) = cast.as_ref().map(|c| &c.decor) else { return };
    if let Some(c) = RENDERER.get(|| None).as_mut().and_then(|r| r.screen.cast.as_mut()) {
        c.grown.unconsumed(d.edits);
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
    blocks().restore(g);
    let (kind, stat) = r.frame(g, world, input);
    FrameOutput {
        kind,
        // An image landing is taken on a turn to come.
        redraw: *REDRAW.get(|| false) || crate::gpu::api::images_landing(),
        stat: Some(stat),
    }
}

/// Blocks listed with no device wait in the table for the next one.
/// The draws and their uniforms, and whether a block was let go, made or
/// rewritten.
fn take_lists<'a>(g: Option<&Gpu>, lists: Lists<'a>) -> (&'a [u32], &'a [f32], bool) {
    let b = blocks();
    // Frees first, so the table never holds a block beside its replacement.
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

#[cfg(test)]
#[path = "renderer.test.rs"]
mod test;
