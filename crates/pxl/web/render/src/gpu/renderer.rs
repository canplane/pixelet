// pxl-render/gpu/renderer.rs

//! The renderer: what the scheduler lists (draw.rs) drawn on the device
//! through the host's calls (api.rs) into the canvases the shell handed the
//! kernel (surface.rs), in the kernel's own turn. Nothing of the draw leaves
//! the module but the calls. A block the scheduler names goes to the GPU
//! whole, as the mesher laid it out; the frame's uniforms are laid out from the
//! stage (stage.rs) and the environment; every pipeline and shader is
//! shader.rs's.
//!
//! What the frame is drawn into is sized here and its cost counted: the depth
//! buffer, the sun's shadow map, the texture pack's layers with their mip chains, the
//! badge sheet. Reversed float depth throughout, near at 1 and far at 0: a
//! float's precision is densest near zero and a perspective depth densest near
//! the eye, so the two cancel and the far terrain's coplanar faces stop
//! fighting.

use pxl_spec::space::mat4::{mat4, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_translation, Mat4};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::v3::{norm3, sub3, V3};
use xpute_core::math::scalar::round;
use xpute_runtime::global::Global;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

use crate::asset::atlas::BillboardAtlas;
use crate::frame::draw::{
    Lists, DRAW_WORDS, PIPELINE_BADGE, PIPELINE_CHUNK, PIPELINE_CHUNK_COARSE, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR,
    PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS, UNIFORM_FLOATS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{FRAME_AT, FRAME_FLOATS, PIPELINES, PIPELINE_CHUNK_COARSE_DEPTH, PIPELINE_CHUNK_COARSE_PREPASS, PIPELINE_CHUNK_DEPTH, PIPELINE_CHUNK_PREPASS, PIPELINE_SKY};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, forget_objects, frame_room, gpu, object, pass_desc, release, room, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{dimension, format, load, resource, surface_role};
use crate::gpu::arena::{arena_buffer, arena_lose};
use crate::gpu::atmosphere::{Atmosphere, AIR_WORDS};
use crate::gpu::clouds::{self, CloudPass};
use crate::gpu::layers::{Layers, Mips};
use crate::gpu::moon::MoonMap;
use crate::gpu::pass::badges::BadgePass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::shadow::Shadow;
use crate::gpu::stars::StarPass;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::look::environment::{ENV_AIR, ENV_OTHER_LIGHT, ENV_STARS};
use crate::look::globe::{Globe, MERCATOR_RADIUS_U};
use crate::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// GL clip z [-1, 1] to reversed WebGPU clip z [1, 0], column-major.
pub const GL_TO_WGPU: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -0.5, 0.0, 0.0, 0.0, 0.5, 1.0];

/// The frame uniform's fields, in floats (shader.rs's FRAME_AT).
pub(crate) const VIEW_PROJ: usize = FRAME_AT[0];
pub(crate) const CAMERA_POS: usize = FRAME_AT[1];
const EDGE_STRENGTH: usize = FRAME_AT[2];
const ENV: usize = FRAME_AT[3];
const LOW_SPEC: usize = FRAME_AT[4];
const EDGE_WIDTH: usize = FRAME_AT[5];
const RIM_INTENSITY: usize = FRAME_AT[6];
const VIEWPORT: usize = FRAME_AT[7];
const SHADOW_STRENGTH: usize = FRAME_AT[8];
const SHADOW_TEXEL: usize = FRAME_AT[9];
const VIGNETTE: usize = FRAME_AT[10];
const AO_STRENGTH: usize = FRAME_AT[11];
const SCREEN_AO: usize = FRAME_AT[12];
const LIGHT_VIEW_PROJ: usize = FRAME_AT[13];
const OTHER: usize = FRAME_AT[14];
pub(crate) const PLANET: usize = FRAME_AT[15];
pub(crate) const GLOBE: usize = FRAME_AT[16];
pub(crate) const AIR_AT_VERTICES: usize = FRAME_AT[17];

/// Floats the sky uniform holds (shader/sky.wgsl's Sky): the ray, the
/// celestial pole, and whether the Moon's face has arrived.
pub(crate) const SKY_FLOATS: usize = 16;
pub(crate) const SKY_POLE: usize = 12;
pub(crate) const SKY_MAPPED: usize = 15;
/// Vertices a chunk mesh's box is drawn with: chunk.wgsl's BOX_VERTICES.
const BOX_VERTICES: u32 = 36;
/// What one box takes in the arena, which is the stride its draw binds at.
const FINE_BOX_BYTES: u32 = FINE_BOX_WORDS * 4;
const COARSE_BOX_BYTES: u32 = COARSE_BOX_WORDS * 4;
/// Floats of the frame's draws storage made before the first frame needs more.
const DRAWS_INITIAL: u32 = 256 * UNIFORM_FLOATS as u32;
const MB: f64 = 1_048_576.0;

/// What a frame's GPU time is taken apart into, in the order the frame
/// encodes them: the air's tables, the sun's shadow map, the depth prepass,
/// the screen-space occlusion from it, the world's own pass, the billboards'
/// and the cases'. The profiler names them in the same order (profiler.json's
/// GPU_SPAN_NAMES), and a pair of queries a span is the frame's set.
pub const GPU_SPANS: [&str; 7] = ["air", "shadow", "prepass", "ao", "world", "badges", "cases"];
const _: () = assert!(GPU_SPANS.len() <= 32, "a span a bit of the mask the host is told");
const SPAN_AIR: usize = 0;
const SPAN_SHADOW: usize = 1;
const SPAN_PREPASS: usize = 2;
const SPAN_AO: usize = 3;
const SPAN_WORLD: usize = 4;
const SPAN_BADGES: usize = 5;
const SPAN_CASES: usize = 6;

pub type Frame = [f32; FRAME_FLOATS];

/// What the kernel hands a turn's draw: every input the frame's passes read
/// that is not the renderer's own.
pub struct FrameInput<'a> {
    /// The draw list's lanes, as the scheduler's `DrawList::take` gave them.
    pub lists: Lists<'a>,
    /// The environment's lane (look/environment.rs), ENV_FLOATS floats.
    pub environment: &'a [f32],
    /// Each decor group's model block, by the list's handle (0 while it
    /// loads), and its skin's image, in group order.
    pub decor: &'a [(u32, Option<u32>)],
    /// The world's actors' models and the images they are skinned with.
    pub skins: &'a [(u32, u32)],
    /// The variant picker's cases: the surface, the model and its skin.
    pub cases: &'a [(u32, u32, u32)],
    /// The texture pack's sheet as an image, asked for until it is drawn.
    pub sheet: fn() -> Option<u32>,
    /// The Moon's face as an image, asked for until it is copied in.
    pub moon: fn() -> Option<u32>,
    /// The billboards' sheet, whose changes the badge texture mirrors.
    pub atlas: &'a mut BillboardAtlas,
    /// A line of text as the platform draws it, asked for until it is.
    pub text: TextOf,
    /// Whether the device is a phone's: the shadow map is made smaller.
    pub handheld: bool,
}

/// What a turn's draw hands back.
pub struct FrameOutput {
    /// A surface waits on a size to settle, so another turn is wanted.
    pub redraw: bool,
    /// What the frame cost, when one was drawn.
    pub stat: Option<FrameStat>,
}

/// A drawn frame's cost: the device's time, the draws, what the device holds.
#[derive(Clone, Copy)]
pub struct FrameStat {
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub buffers: f64,
    pub buffer_bytes: f64,
    /// The targets' and textures' megabytes: depth, the shadow map, the
    /// texture pack's layers.
    pub depth_mb: f64,
    pub shadow_mb: f64,
    pub layers_mb: f64,
    /// The world surface's width times its height.
    pub px: f64,
    /// Draws and triangles, by pipeline.
    pub census: [[f64; 2]; PIPELINES as usize],
    /// The GPU's time by span, GPU_SPANS' order; `gpu_ms` is theirs end to end.
    pub gpu_spans: [f64; GPU_SPANS.len()],
    /// What the mesh arena holds on the device, which is no block's.
    pub arena_mb: f64,
}

pub fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_mat(out: &mut [f32], at: usize, m: &Mat4) {
    for (i, v) in m.iter().enumerate() {
        out[at + i] = *v as f32;
    }
}

/// A pixel's ray for a full-screen pass, as the three columns of a WGSL
/// `mat3x3f` at `at`: the ray under NDC (x, y) runs along
/// `x·col0 + y·col1 + col2`, in the frame `view_proj` takes to WebGPU clip,
/// from the eye at `eye` in it.
///
/// Not the inverse matrix itself, un-projecting two points in the shader:
/// those sit a near plane's tenth of a unit apart, hundreds of units from
/// the frame's origin, and their f32 difference kept a direction to about a
/// thousandth of a radian that changed its rounding with every move of the
/// camera. A ray meeting the cloud layer at a grazing angle carries that
/// error out by the secant — kilometers at the horizon — and the clouds
/// there shook on their own. Taken in f64 here, the ray is exact to the f32
/// it lands in.
/// Points at one depth un-project onto a plane, so the ray is affine in
/// (x, y) and three columns hold it.
pub(crate) fn put_ray(out: &mut [f32], at: usize, view_proj: &Mat4, eye: &V3) {
    let mut inv = mat4();
    mat4_invert(&mut inv, view_proj);
    let point = |x: f64, y: f64| -> V3 {
        let clip = [x, y, 0.5, 1.0];
        let row = |r: usize| (0..4).map(|c| inv[c * 4 + r] * clip[c]).sum::<f64>();
        let w = row(3);
        [row(0) / w, row(1) / w, row(2) / w]
    };
    let c = point(0.0, 0.0);
    let mut column = [0.0; 3];
    for (k, (to, from)) in [(point(1.0, 0.0), c), (point(0.0, 1.0), c), (c, *eye)].into_iter().enumerate() {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3].iter_mut().zip(sub3(&to, &from, &mut column).iter()) {
            *o = *e as f32;
        }
    }
}

/// A frame group: the frame uniform at 0, a shadow map's two entries, the
/// occlusion map and the air's five (gpu/atmosphere.rs).
pub fn frame_group(g: &Gpu, layout: u32, frame: u32, shadow: [u32; 10], ao: [u32; 5], air: [u32; AIR_WORDS]) -> u32 {
    let id = object();
    let mut e = [0u32; 20 + AIR_WORDS];
    e[..5].copy_from_slice(&buffer_entry(0, frame));
    e[5..15].copy_from_slice(&shadow);
    e[15..20].copy_from_slice(&ao);
    e[20..].copy_from_slice(&air);
    g.bind_group(id, layout, &e);
    id
}

/// A skin's texture and its sampler as a group, over the image `image`. The
/// texture, its view and the group.
pub fn skin_group(g: &Gpu, layout: u32, sampler: u32, image: u32) -> [u32; 3] {
    let texture = object();
    g.image_texture(texture, image);
    let view = object();
    g.view(view, texture, dimension::D2, 0, 1, 0, 1);
    let group = object();
    g.bind_group(group, layout, &[0, resource::VIEW, view, 0, 0, 1, resource::SAMPLER, sampler, 0, 0]);
    [texture, view, group]
}

/// A heap block words are laid out in before they go to the
/// device: grown to the next power of two when a frame needs more, never shrunk.
#[derive(Default)]
pub struct Scratch {
    at: usize,
    bytes: u32,
}

impl Scratch {
    pub fn take(&mut self, bytes: u32) -> &'static mut [u8] {
        if bytes > self.bytes {
            if self.bytes > 0 {
                // SAFETY: the block this scratch held alone.
                unsafe { free((self.at, self.bytes)) };
            }
            let size = bytes.next_power_of_two().max(4096);
            (self.at, self.bytes) = alloc(size).or_bug(Errno::ENOMEM);
        }
        // SAFETY: a heap block this scratch holds alone, at
        // least `bytes` long.
        unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(self.at), bytes as usize) }
    }

    /// Gives the block back.
    pub fn free(&mut self) {
        if self.bytes > 0 {
            // SAFETY: the block this scratch held alone.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
        }
    }
}

const BLOCK_SLOTS: usize = 1 << 15;
const BLOCK_MASK: usize = BLOCK_SLOTS - 1;
const EMPTY: u32 = 0;

/// The blocks the draw list named, by the handle it gave each: where the block
/// lies in the memory and its counts, and once a device has it the buffer and
/// the one group made over it — the decor instances, a joint palette or the
/// puck's zone, whichever the block is. The memory is the source and the
/// device a copy: a block stays in the heap until its free is listed, so a
/// device that went away, or lists taken while there was none, are made good
/// from the addresses. Open addressing in `.fixed.blocks`'s lanes, since the
/// handles are the list's and sparse.
pub struct Blocks {
    handle: &'static mut [u32],
    at: &'static mut [u32],
    len: &'static mut [u32],
    vertex_cnt: &'static mut [u32],
    index_cnt: &'static mut [u32],
    buffer: &'static mut [u32],
    group: &'static mut [u32],
    pub count: u32,
    pub bytes: u64,
    /// Live blocks with no buffer on the device the renderer has.
    missing: u32,
}

/// A block's buffer size: its bytes to the word, never empty.
fn buffer_size(len: u32) -> u32 {
    ((len + 3) & !3).max(4)
}

/// `n` bytes of the memory at `at`.
fn memory(at: u32, n: u32) -> &'static [u8] {
    // SAFETY: a heap block the draw list named, live until its free is
    // listed.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at as usize), n as usize) }
}

/// Lanes the table is: handle, where, length, the two counts, buffer, group.
const BLOCK_LANES: usize = 7;

/// `.fixed.blocks`, the table's lanes end to end: the kernel lays it out
/// (pxl-kernel mem/fixed.rs) and installs it with `set_blocks`.
pub const BLOCKS_BYTES: u32 = (BLOCK_LANES * BLOCK_SLOTS * 4) as u32;

/// Installs the table in the `BLOCKS_BYTES` at `at`.
///
/// # Safety
/// `at` must address that range, 4-aligned and zeroed, the renderer's alone
/// for as long as the module runs.
pub unsafe fn set_blocks(at: u32) {
    // SAFETY: the caller's range, one lane of BLOCK_SLOTS words after another.
    let lane = |k: usize| unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize + k * BLOCK_SLOTS * 4), BLOCK_SLOTS) };
    BLOCKS.set(Blocks {
        handle: lane(0),
        at: lane(1),
        len: lane(2),
        vertex_cnt: lane(3),
        index_cnt: lane(4),
        buffer: lane(5),
        group: lane(6),
        count: 0,
        bytes: 0,
        missing: 0,
    });
}

fn home(handle: u32) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - BLOCK_SLOTS.trailing_zeros())) as usize
}

impl Blocks {
    /// The device went away with every buffer and group: the blocks stand,
    /// each to be put on the next one.
    fn lose(&mut self) {
        self.buffer.fill(0);
        self.group.fill(0);
        self.missing = self.count;
    }

    /// Puts every block that has no buffer on the device, as far as the
    /// turn's stream has room.
    fn restore(&mut self, g: &Gpu) {
        if self.missing == 0 {
            return;
        }
        for i in 0..BLOCK_SLOTS {
            if self.handle[i] != EMPTY && self.buffer[i] == 0 && !self.put(g, i) {
                return;
            }
        }
    }

    /// A buffer for the block at `i`, its bytes written; false, and the
    /// block still missing, where the turn's stream has no room for them.
    fn put(&mut self, g: &Gpu, i: usize) -> bool {
        if !room(self.len[i]) {
            return false;
        }
        let buffer = object();
        // Every block is usable every way a draw reads one: a mesh's lanes as
        // vertices and indices, a decor block's as storage.
        g.buffer(
            buffer,
            buffer_size(self.len[i]),
            buffer_usage::VERTEX | buffer_usage::INDEX | buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buffer(buffer, 0, memory(self.at[i], self.len[i] & !3));
        self.buffer[i] = buffer;
        self.group[i] = 0;
        self.missing -= 1;
        true
    }

    fn find(&self, handle: u32) -> Option<usize> {
        if handle == EMPTY {
            return None;
        }
        let mut i = home(handle);
        loop {
            match self.handle[i] {
                EMPTY => return None,
                h if h == handle => return Some(i),
                _ => i = (i + 1) & BLOCK_MASK,
            }
        }
    }

    /// A block's buffer and its vertex and index counts; none until the block
    /// is on the device.
    pub fn get(&self, handle: u32) -> Option<(u32, u32, u32)> {
        self.find(handle).filter(|&i| self.buffer[i] != 0).map(|i| (self.buffer[i], self.vertex_cnt[i], self.index_cnt[i]))
    }

    /// Records a block, and puts it on the device when there is one; what the
    /// handle held before is let go.
    fn insert(&mut self, g: Option<&Gpu>, handle: u32, at: u32, len: u32, vertex_cnt: u32, index_cnt: u32) {
        self.remove(handle);
        xpute_core::ensure!((self.count as usize) < BLOCK_SLOTS / 2, ENOSPC, BLOCK_SLOTS / 2);
        let mut i = home(handle);
        while self.handle[i] != EMPTY {
            i = (i + 1) & BLOCK_MASK;
        }
        self.handle[i] = handle;
        self.at[i] = at;
        self.len[i] = len;
        self.vertex_cnt[i] = vertex_cnt;
        self.index_cnt[i] = index_cnt;
        self.buffer[i] = 0;
        self.group[i] = 0;
        self.count += 1;
        self.bytes += buffer_size(len) as u64;
        self.missing += 1;
        if let Some(g) = g {
            self.put(g, i);
        }
    }

    /// The block's bytes changed at `at`: written into its buffer when it has
    /// one, and read from there when it is next put on a device. Where the
    /// turn's stream has no room for them, the buffer goes and the block is
    /// missing, put again whole once there is.
    fn rewrite(&mut self, g: Option<&Gpu>, handle: u32, at: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.at[i] = at;
        let (Some(g), buffer @ 1..) = (g, self.buffer[i]) else { return };
        let bytes = len.min(self.len[i]) & !3;
        if room(bytes) {
            g.write_buffer(buffer, 0, memory(at, bytes));
        } else {
            release(self.group[i]);
            release(buffer);
            (self.buffer[i], self.group[i]) = (0, 0);
            self.missing += 1;
        }
    }

    /// Forgets a block, its buffer and group let go.
    fn remove(&mut self, handle: u32) {
        let Some(mut i) = self.find(handle) else { return };
        release(self.group[i]);
        if self.buffer[i] == 0 {
            self.missing = self.missing.saturating_sub(1);
        }
        release(self.buffer[i]);
        self.count -= 1;
        self.bytes -= buffer_size(self.len[i]) as u64;
        // Backward shift: an entry past the hole moves into it unless its home
        // lies between the hole and it, so no probe ever stops early.
        loop {
            let mut j = i;
            loop {
                j = (j + 1) & BLOCK_MASK;
                if self.handle[j] == EMPTY {
                    self.handle[i] = EMPTY;
                    return;
                }
                let k = home(self.handle[j]);
                let stays = if i <= j { i < k && k <= j } else { i < k || k <= j };
                if !stays {
                    self.handle[i] = self.handle[j];
                    self.at[i] = self.at[j];
                    self.len[i] = self.len[j];
                    self.vertex_cnt[i] = self.vertex_cnt[j];
                    self.index_cnt[i] = self.index_cnt[j];
                    self.buffer[i] = self.buffer[j];
                    self.group[i] = self.group[j];
                    i = j;
                    break;
                }
            }
        }
    }

    /// The group over a block's buffer in `layout`, made on first use.
    /// A block's buffer, for a draw that binds it as vertex rather than as a
    /// group: the ghost's one box goes through the chunk pipeline, whose mesh
    /// is a vertex buffer now (gpu/arena.rs).
    pub fn buffer(&self, handle: u32) -> Option<u32> {
        self.find(handle).filter(|&i| self.buffer[i] != 0).map(|i| self.buffer[i])
    }

    pub fn group(&mut self, g: &Gpu, handle: u32, layout: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.buffer[i] != 0)?;
        if self.group[i] == 0 {
            let id = object();
            g.bind_group(id, layout, &buffer_entry(0, self.buffer[i]));
            self.group[i] = id;
        }
        Some(self.group[i])
    }
}

static BLOCKS: Global<Blocks> = Global::new();

/// The table, which the module installs at boot (`set_blocks`).
fn blocks() -> &'static mut Blocks {
    BLOCKS.get(|| xpute_core::bug!(ENOTRECOVERABLE))
}

#[derive(Clone, Copy)]
struct Device {
    align: u32,
    timestamps: bool,
}

static DEVICE: Global<Option<Device>> = Global::new();
static RENDERER: Global<Option<Renderer>> = Global::new();

/// Set when a surface waits a turn for its size to settle: the draw's
/// answer that another turn is wanted.
static REDRAW: Global<bool> = Global::new();

/// The host's report that a device answered: its uniform offset alignment, and
/// whether it measures its passes. The caller asks for the turn that draws.
pub fn gpu_created(align: u32, timestamps: bool) {
    DEVICE.set(Some(Device { align, timestamps }));
}

/// Whether a device answered and has not gone.
pub fn device_ready() -> bool {
    DEVICE.get(|| None).is_some()
}

/// The host's report that the device went away, and everything on it.
pub fn gpu_lost() {
    DEVICE.set(None);
    RENDERER.set(None);
    forget_objects();
    blocks().lose();
    arena_lose();
    surfaces_lost();
}

/// A surface closed: what it held on the device let go.
pub fn surface_closed(s: Surface) {
    for id in [s.depth_view, s.depth] {
        release(id);
    }
    let slot = xpute_runtime::abi::handle::handle_slot(s.handle) as usize;
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        if let Some(case) = r.cases[slot].take() {
            case.release();
        }
    }
}

struct DebugPass {
    built: Built,
    draw_buffer: u32,
    frame_group: u32,
    draw_group: u32,
}

impl DebugPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, format::CANVAS);
        let draw_buffer = object();
        g.buffer(draw_buffer, 16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let frame_group = object();
        g.bind_group(frame_group, built.layouts[0], &buffer_entry(0, frame_buffer));
        let draw_group = object();
        g.bind_group(draw_group, built.layouts[1], &buffer_entry(0, draw_buffer));
        DebugPass {
            built,
            draw_buffer,
            frame_group,
            draw_group,
        }
    }

    /// The frame's segments into the open pass; how many vertices.
    fn draw(&self, g: &Gpu, pass: u32, draws: &[u32], uniforms: &[f32], blocks: &Blocks) -> u32 {
        let Some(d) = draws.as_chunks::<DRAW_WORDS>().0.iter().find(|d| d[4] == PIPELINE_DEBUG) else {
            return 0;
        };
        let (Some((block, _, _)), count) = (blocks.get(d[0]), d[1]) else { return 0 };
        if count == 0 {
            return 0;
        }
        let at = d[3] as usize * UNIFORM_FLOATS;
        g.write_buffer(self.draw_buffer, 0, bytes(&[uniforms[at], uniforms[at + 1], uniforms[at + 2], 0.0]));
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        g.set_bind_group(pass, 1, self.draw_group, &[]);
        g.set_vertex_buffer(pass, 0, block, 0, count * 24);
        g.draw(pass, count, 1, 0, 0);
        count
    }
}

/// A case of the variant picker: its own frame, and the skinned pass that
/// draws its one actor into it at one sample, with no shadow.
struct Case {
    frame_buffer: u32,
    frame: Frame,
    skinned: SkinnedPass,
}

/// The eye and what it looks at, in a case's own Z-up frame, and its field.
const CASE_EYE: V3 = [0.0, -2.2, 0.9];
const CASE_TARGET: V3 = [0.0, 0.0, 0.55];
const CASE_FOV_DEG: f64 = 35.0;

impl Case {
    fn new(g: &Gpu, pipelines: &mut Pipelines, shadow: &Shadow, ao: &ScreenAo, air: [u32; AIR_WORDS], align: u32) -> Case {
        let frame_buffer = object();
        g.buffer(frame_buffer, word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let skinned = SkinnedPass::new(g, pipelines, frame_buffer, shadow.blank_entries(1), None, ao.blank_entry(7), air, align);
        Case {
            frame_buffer,
            frame: [0.0; FRAME_FLOATS],
            skinned,
        }
    }

    fn release(self) {
        release(self.frame_buffer);
        self.skinned.release();
    }
}

pub struct Renderer {
    align: u32,
    query_set: u32,
    pipelines: Pipelines,
    frame_buffer: u32,
    frame: Frame,
    sky_buffer: u32,
    sky: [f32; SKY_FLOATS],
    sky_built: Built,
    sky_group: u32,
    chunk: Built,
    chunk_depth: Built,
    chunk_prepass: Built,
    /// The same three over the coarse box format, which differs from the
    /// leaf's own in stride alone (frame/shader/record.rs's chunk_lanes).
    chunk_coarse: Built,
    chunk_coarse_depth: Built,
    chunk_coarse_prepass: Built,
    ghost: [Built; 2],
    draws_buffer: u32,
    draws_bytes: u32,
    bind_group: u32,
    light_group: u32,
    shadow: Shadow,
    shadows: bool,
    ao: ScreenAo,
    screen_ao: bool,
    atmosphere: Atmosphere,
    stars: StarPass,
    clouds: CloudPass,
    mips: Mips,
    layers: Layers,
    moon: MoonMap,
    decor: DecorPass,
    skinned: SkinnedPass,
    puck: PuckPass,
    badges: BadgePass,
    debug: DebugPass,
    cases: [Option<Case>; SURFACES],
    gpu_ms: f64,
    gpu_spans: [f64; GPU_SPANS.len()],
}

impl Renderer {
    fn new(g: &Gpu, device: Device, handheld: bool) -> Renderer {
        let mut pipelines = Pipelines::new();
        let frame_buffer = object();
        g.buffer(frame_buffer, word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let sky_buffer = object();
        g.buffer(sky_buffer, word_bytes(SKY_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let shadow = Shadow::new(g, handheld);
        let ao = ScreenAo::new(g, &mut pipelines, device.align);
        let atmosphere = Atmosphere::new(g, &mut pipelines);
        let air = atmosphere.entries();
        let stars = StarPass::new(g, &mut pipelines);
        let clouds = CloudPass::new(g, &mut pipelines, frame_buffer, air);
        let mips = Mips::new(g, &mut pipelines, format::RGBA8_SRGB);
        let layers = Layers::new(g);
        let chunk = pipelines.build(g, PIPELINE_CHUNK, format::CANVAS);
        let sky_built = pipelines.build(g, PIPELINE_SKY, format::CANVAS);
        let sky_group = object();
        let moon = MoonMap::new(g);
        let mut sky_entries = [0u32; 20 + AIR_WORDS];
        sky_entries[..5].copy_from_slice(&buffer_entry(0, frame_buffer));
        sky_entries[5..10].copy_from_slice(&buffer_entry(1, sky_buffer));
        sky_entries[10..15].copy_from_slice(&[2, resource::VIEW, moon.view, 0, 0]);
        sky_entries[15..20].copy_from_slice(&[3, resource::SAMPLER, moon.sampler, 0, 0]);
        sky_entries[20..].copy_from_slice(&air);
        g.bind_group(sky_group, sky_built.layouts[0], &sky_entries);
        let draws_buffer = object();
        g.buffer(draws_buffer, DRAWS_INITIAL * 4, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        let query_set = if device.timestamps {
            let q = object();
            g.query_set(q, 2 * GPU_SPANS.len() as u32);
            q
        } else {
            0
        };
        let mut r = Renderer {
            align: device.align,
            query_set,
            chunk,
            chunk_depth: pipelines.build(g, PIPELINE_CHUNK_DEPTH, format::CANVAS),
            chunk_prepass: pipelines.build(g, PIPELINE_CHUNK_PREPASS, format::CANVAS),
            chunk_coarse: pipelines.build(g, PIPELINE_CHUNK_COARSE, format::CANVAS),
            chunk_coarse_depth: pipelines.build(g, PIPELINE_CHUNK_COARSE_DEPTH, format::CANVAS),
            chunk_coarse_prepass: pipelines.build(g, PIPELINE_CHUNK_COARSE_PREPASS, format::CANVAS),
            ghost: [pipelines.build(g, PIPELINE_GHOST_BOX, format::CANVAS), pipelines.build(g, PIPELINE_GHOST_BOX_COPLANAR, format::CANVAS)],
            decor: DecorPass::new(g, &mut pipelines, frame_buffer, &shadow, ao.blank_entry(7), air, device.align),
            skinned: SkinnedPass::new(
                g,
                &mut pipelines,
                frame_buffer,
                shadow.entries(1),
                Some((shadow.light_frame, shadow.blank_entries(1))),
                ao.blank_entry(7),
                air,
                device.align,
            ),
            puck: PuckPass::new(g, &mut pipelines, frame_buffer),
            badges: BadgePass::new(g, &mut pipelines),
            debug: DebugPass::new(g, &mut pipelines, frame_buffer),
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
            light_group: 0,
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
            gpu_ms: 0.0,
            gpu_spans: [0.0; GPU_SPANS.len()],
        };
        r.bind_group = r.chunk_group(g, r.frame_buffer, r.shadow.entries(1));
        r.light_group = r.chunk_group(g, r.shadow.light_frame, r.shadow.blank_entries(1));
        r
    }

    /// The chunk pass's group over `frame`: the frame, a shadow map, the draws,
    /// the texture pack's array, the table its faces are found through and
    /// their sampler, the occlusion map, the air.
    fn chunk_group(&self, g: &Gpu, frame: u32, shadow: [u32; 10]) -> u32 {
        let mut e = [0u32; 40 + AIR_WORDS];
        e[..5].copy_from_slice(&buffer_entry(0, frame));
        e[5..15].copy_from_slice(&shadow);
        e[15..20].copy_from_slice(&buffer_entry(3, self.draws_buffer));
        e[20..25].copy_from_slice(&[4, resource::VIEW, self.layers.view, 0, 0]);
        e[25..30].copy_from_slice(&buffer_entry(5, self.layers.table));
        e[30..35].copy_from_slice(&[6, resource::SAMPLER, self.layers.sampler, 0, 0]);
        e[35..40].copy_from_slice(&self.ao.entry(7));
        e[40..].copy_from_slice(&self.atmosphere.entries());
        let id = object();
        g.bind_group(id, self.chunk.layouts[0], &e);
        id
    }

    /// Every lit pass's group 0 again: the occlusion map is a new object.
    fn rebind(&mut self, g: &Gpu) {
        release(self.bind_group);
        release(self.light_group);
        self.bind_group = self.chunk_group(g, self.frame_buffer, self.shadow.entries(1));
        self.light_group = self.chunk_group(g, self.shadow.light_frame, self.shadow.blank_entries(1));
        let ao = self.ao.entry(7);
        self.decor.rebind(g, self.frame_buffer, &self.shadow, ao);
        self.skinned
            .rebind(g, self.frame_buffer, self.shadow.entries(1), Some((self.shadow.light_frame, self.shadow.blank_entries(1))), ao);
    }

    /// The frame's uniforms and the sky's, laid out from the stage and the
    /// environment `env` for the world's surface at (width, height) device
    /// pixels.
    fn lay_out(&mut self, g: &Gpu, width: u32, height: u32, env: &[f32]) {
        let s = stage_lane();
        let (mut a, mut m, mut proj) = (mat4(), mat4(), mat4());
        a.copy_from_slice(&s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16]);
        mat4_mul(&mut proj, &GL_TO_WGPU, &a);
        // The badges take the view and the projection apart: their size is
        // measured in view space.
        let b = &mut self.badges;
        put_mat(&mut b.view, 0, s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].try_into().or_bug(Errno::ENOTRECOVERABLE));
        put_mat(&mut b.proj, 0, &proj);
        b.px_k = (s[STAGE_AT::VIEWPORT + 1] / (2.0 * (s[STAGE_AT::FOV] * core::f64::consts::PI / 360.0).tan())) as f32;
        b.fade_start = s[STAGE_AT::FADE_START] as f32;
        b.fade_end = s[STAGE_AT::FADE_END] as f32;
        b.near = s[STAGE_AT::NEAR] as f32;
        b.far = s[STAGE_AT::FAR] as f32;
        b.cell = s[STAGE_AT::LOD_CELL] as f32;

        let u = &mut self.frame;
        a.copy_from_slice(&s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16]);
        mat4_mul(&mut m, &GL_TO_WGPU, &a);
        put_mat(u, VIEW_PROJ, &m);
        // Everything is drawn in the eye's frame, the root frame with the eye
        // at its origin (frame/draw.rs `DrawList::translation`): the meshes,
        // the sky and the stars, each pixel's ray from the eye, and the
        // clouds, anchored to the ground by the cell the eye is over on the
        // map. What stands anywhere in it — the globe's anchor below — is
        // written against the eye in doubles before it is narrowed.
        let eye_local = [s[STAGE_AT::EYE_LOCAL], s[STAGE_AT::EYE_LOCAL + 1], s[STAGE_AT::EYE_LOCAL + 2]];
        let eye = [0.0; 3];
        let origin = ivec::gather(ivec::from_real(s[STAGE_AT::ORIGIN]), ivec::from_real(s[STAGE_AT::ORIGIN + 1]), 0);
        // The globe's anchor, which the sun's shadow is aimed at and the map
        // is laid back on the planet about (look/globe.rs): its latitude's
        // values and the planet's radius in the units the map has there. The
        // globe's placement is a function of a point's offset from it and its
        // height over it alone, so it places a point of the eye's frame where
        // it places the same point of the root frame, less the eye.
        let center: [f64; 3] = core::array::from_fn(|k| s[STAGE_AT::GLOBE_ANCHOR + k] - eye_local[k]);
        let sphere = Globe::at(env[ENV_AIR + 2] as f64);
        // Straight up where the eye stands: away from the planet's center,
        // the radius below the globe's anchor, which is under the eye — the
        // air, the clouds and the stars are measured from it, on the sphere
        // the world is laid on.
        let planet_center = [center[0], center[1], center[2] - sphere.radius_u()];
        let mut up = [0.0; 3];
        norm3(sub3(&eye, &planet_center, &mut [0.0; 3]), &mut up);
        self.clouds.lay_out(g, &m, &eye, env, &up);
        self.atmosphere.set_clouds(&clouds::field(origin, [eye_local[0], eye_local[1]], env));
        self.stars.lay_out(g, &m, env, width, height, &up);
        put_ray(&mut self.sky, 0, &m, &eye);
        // The celestial pole in the root frame, the stars' frame's third
        // column: which way the Moon's north is turned.
        self.sky[SKY_POLE..SKY_POLE + 3].copy_from_slice(&env[ENV_STARS + 8..ENV_STARS + 11]);
        self.sky[SKY_MAPPED] = if self.moon.loaded { 1.0 } else { 0.0 };
        // The camera's position in the eye's frame, where the meshes are.
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
        }
        u[EDGE_STRENGTH] = s[STAGE_AT::EDGE_STRENGTH] as f32;
        u[ENV..ENV + 24].copy_from_slice(&env[0..24]);
        u[OTHER..OTHER + 8].copy_from_slice(&env[ENV_OTHER_LIGHT..ENV_OTHER_LIGHT + 8]);
        u[LOW_SPEC] = s[STAGE_AT::LOW_SPEC] as f32;
        u[EDGE_WIDTH] = s[STAGE_AT::EDGE_WIDTH] as f32;
        u[RIM_INTENSITY] = s[STAGE_AT::RIM_INTENSITY] as f32;
        u[VIEWPORT] = width as f32;
        u[VIEWPORT + 1] = height as f32;
        u[VIGNETTE] = 1.0;
        u[AIR_AT_VERTICES] = 1.0;
        u[AO_STRENGTH] = s[STAGE_AT::AO_STRENGTH] as f32;
        self.screen_ao = s[STAGE_AT::SCREEN_AO] != 0.0;
        u[SCREEN_AO] = if self.screen_ao { 1.0 } else { 0.0 };
        if self.screen_ao {
            self.ao.lay_out(g, &proj, width, height, env[7], env[11]);
        }
        let globe = [
            center[0] as f32,
            center[1] as f32,
            sphere.radius_u() as f32,
            center[2] as f32,
            sphere.e0 as f32,
            sphere.sin_lat as f32,
            sphere.cos_lat as f32,
            MERCATOR_RADIUS_U as f32,
        ];
        u[PLANET..PLANET + 4].copy_from_slice(&globe[..4]);
        u[GLOBE..GLOBE + 4].copy_from_slice(&globe[4..]);
        self.badges.globe = globe;
        // The sun's shadow, from the light the environment lit the terrain with.
        self.shadows = s[STAGE_AT::SHADOWS] != 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_TEXEL] = self.shadow.texel as f32;
        if self.shadows {
            self.shadow.aim(g, &env[0..3], center, globe);
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
        }

        self.atmosphere.lay_out(g, env, &up);
    }

    /// Every chunk-pipeline draw of pipeline `which`; with `own`, that pipeline
    /// set before the first. Whether any was drawn.
    /// A mesh is its boxes, read by the vertex shader from the buffer at
    /// group 1, a cube's worth of vertices each (chunk.wgsl): a chunk's in its
    /// arena group from its first box, the hover ghost's in a block of its own.
    /// Every chunk-pipeline draw of pipeline `which`, whose boxes are `stride`
    /// bytes each; with `own`, that pipeline set before the first. Whether any
    /// was drawn.
    ///
    /// A box is one instance and the thirty-six vertices of a cube share it,
    /// so the mesh is bound as a vertex buffer and the draw's own first box is
    /// the offset it is bound at. Nothing else may carry that offset: an
    /// instance-stepped attribute is fetched at `first_instance + i`, so a
    /// first instance would be added to it rather than read beside it. The
    /// draw record's index goes on the first vertex instead, which no
    /// attribute here steps by (chunk.wgsl).
    fn draw_meshes(&self, g: &Gpu, pass: u32, draws: &[u32], which: u32, stride: u32, own: Option<u32>) -> bool {
        let b = blocks();
        let mut any = false;
        for d in draws.as_chunks::<DRAW_WORDS>().0 {
            let (handle, box_cnt, first_box, uniform, pipeline) = (d[0], d[1], d[2], d[3], d[4]);
            if pipeline != which {
                continue;
            }
            // A chunk's mesh is in the arena, its first word the group; the
            // hover ghost's is a block of its own.
            let buffer = if matches!(pipeline, PIPELINE_CHUNK | PIPELINE_CHUNK_COARSE) {
                arena_buffer(handle as usize)
            } else {
                b.buffer(handle)
            };
            // A draw naming a buffer that is not there is a fault in the list
            // and not a frame to get through. Skipping it drew nothing and
            // said nothing, and the census counted it drawn either way — which
            // is how a device came to draw no ground at all with every gauge
            // reading normal. Saying so here is also what lets the census be
            // taken from the list: every draw listed is a draw issued.
            let Some(buffer) = buffer else { xpute_core::bug!(ENOTRECOVERABLE, handle, pipeline) };
            if let (Some(p), false) = (own, any) {
                g.set_pipeline(pass, p);
                g.set_bind_group(pass, 0, self.bind_group, &[]);
            }
            any = true;
            // Both slots over the one buffer: the second reads the two words a
            // coarse box does not have, which its layout puts back at offset
            // zero so that one shader serves both formats (chunk.wgsl).
            // The run's own start is the buffer's offset and not a first
            // instance: an instance-stepped attribute is fetched at
            // `first_instance + i`, so the two would add up. That leaves the
            // first vertex free to carry the draw record's index, which the
            // shader divides back out (chunk.wgsl).
            let at = first_box * stride;
            g.set_vertex_buffer(pass, 0, buffer, at, 0);
            g.set_vertex_buffer(pass, 1, buffer, at, 0);
            g.draw(pass, BOX_VERTICES, box_cnt, uniform * BOX_VERTICES, 0);
        }
        any
    }

    /// The world, on the world's surface, and every case on its own; what the
    /// frame cost.
    fn frame(&mut self, g: &Gpu, world: &mut Surface, input: FrameInput) -> FrameStat {
        self.decor.set_models(input.decor);
        for &(model, image) in input.skins {
            self.skinned.set_skin(g, model, image);
        }
        self.layers.step(g, &self.mips, input.sheet);
        self.moon.step(g, &self.mips, input.moon);
        self.badges.sync_sheet(g, &self.mips, input.atlas, input.text);
        let (draws, uniforms) = take_lists(Some(g), input.lists);

        targets(g, world);
        self.lay_out(g, world.width, world.height, input.environment);
        if self.screen_ao && self.ao.ensure(g, world.width, world.height) {
            self.rebind(g);
        }
        g.write_buffer(self.frame_buffer, 0, bytes(&self.frame));
        g.write_buffer(self.sky_buffer, 0, bytes(&self.sky));
        let uniform_bytes = word_bytes(uniforms.len());
        if uniform_bytes > self.draws_bytes {
            release(self.bind_group);
            release(self.light_group);
            release(self.draws_buffer);
            self.draws_bytes = uniform_bytes * 2;
            self.draws_buffer = object();
            g.buffer(self.draws_buffer, self.draws_bytes, buffer_usage::STORAGE | buffer_usage::COPY_DST);
            self.bind_group = self.chunk_group(g, self.frame_buffer, self.shadow.entries(1));
            self.light_group = self.chunk_group(g, self.shadow.light_frame, self.shadow.blank_entries(1));
        }
        if !uniforms.is_empty() {
            g.write_buffer(self.draws_buffer, 0, bytes(uniforms));
        }
        self.decor.prepare(g, draws, uniforms);
        self.skinned.prepare(g, draws, uniforms);

        let b = blocks();
        let encoder = object();
        g.encoder(encoder);
        let q = self.query_set;
        // The air's tables, where any is due, before any pass reads them.
        // A span a pair of queries, each written only by the passes the frame
        // ran: which were is told with the set, since a query no pass wrote
        // holds whatever an earlier frame left in it.
        let mut stamps: [Stamp; GPU_SPANS.len()] = core::array::from_fn(|span| Stamp::of(q, span));
        self.atmosphere.draw(g, encoder, &mut stamps[SPAN_AIR]);
        // The sun's shadow map first, from the frame's lists, where it is on.
        if self.shadows {
            let pass = self.shadow.begin(g, encoder, &mut stamps[SPAN_SHADOW]);
            g.set_pipeline(pass, self.chunk_depth.pipeline);
            g.set_bind_group(pass, 0, self.light_group, &[]);
            self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, FINE_BOX_BYTES, None);
            g.set_pipeline(pass, self.chunk_coarse_depth.pipeline);
            self.draw_meshes(g, pass, draws, PIPELINE_CHUNK_COARSE, COARSE_BOX_BYTES, None);
            self.decor.draw_depth(g, pass, draws, b);
            self.skinned.draw_depth(g, pass, draws, b);
            g.end(pass);
            release(pass);
        }
        // The world's depth from the eye, and the screen-space occlusion
        // from it, where that mode is on: the lit passes read the map.
        if self.screen_ao {
            let pass = self.ao.begin_prepass(g, encoder, &mut stamps[SPAN_PREPASS]);
            g.set_pipeline(pass, self.chunk_prepass.pipeline);
            g.set_bind_group(pass, 0, self.bind_group, &[]);
            self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, FINE_BOX_BYTES, None);
            g.set_pipeline(pass, self.chunk_coarse_prepass.pipeline);
            self.draw_meshes(g, pass, draws, PIPELINE_CHUNK_COARSE, COARSE_BOX_BYTES, None);
            self.decor.draw_prepass(g, pass, draws, b);
            self.skinned.draw_prepass(g, pass, draws, b);
            g.end(pass);
            release(pass);
            self.ao.run(g, encoder, &mut stamps[SPAN_AO]);
        }
        let canvas_view = object();
        g.current(world.handle, canvas_view);
        let pass = object();
        g.pass(pass, encoder, &pass_desc(canvas_view, load::CLEAR, world.depth_view, load::CLEAR, 0.0, stamps[SPAN_WORLD].pass()));
        g.set_pipeline(pass, self.chunk.pipeline);
        g.set_bind_group(pass, 0, self.bind_group, &[]);
        self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, FINE_BOX_BYTES, None);
        g.set_pipeline(pass, self.chunk_coarse.pipeline);
        self.draw_meshes(g, pass, draws, PIPELINE_CHUNK_COARSE, COARSE_BOX_BYTES, None);
        self.decor.draw(g, pass, draws, b);
        self.skinned.draw(g, pass, draws, b);
        // The sky and the stars after the opaque world, at the far plane, so
        // only a pixel it left empty runs the sky's shader: looking down at the
        // map, none does.
        g.set_pipeline(pass, self.sky_built.pipeline);
        g.set_bind_group(pass, 0, self.sky_group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.stars.draw(g, pass);
        // The clouds over the opaque world and the sky: from under the layer
        // where the world left the sky, from over it over everything
        // (gpu/clouds.rs).
        self.clouds.draw(g, pass);
        self.puck.draw_beacon(g, pass, draws, uniforms);
        self.puck.draw_zone(g, pass, draws, uniforms, b);
        // What is blended goes last, over everything opaque.
        self.draw_meshes(g, pass, draws, PIPELINE_GHOST_BOX, FINE_BOX_BYTES, Some(self.ghost[0].pipeline));
        self.draw_meshes(g, pass, draws, PIPELINE_GHOST_BOX_COPLANAR, FINE_BOX_BYTES, Some(self.ghost[1].pipeline));
        self.decor.draw_ghosts(g, pass, draws, b);
        self.debug.draw(g, pass, draws, uniforms, b);
        g.end(pass);
        release(pass);
        let badges = self
            .badges
            .draw(g, encoder, canvas_view, world.depth_view, [world.width, world.height], draws, uniforms, b, &mut stamps[SPAN_BADGES]);
        self.draw_cases(g, encoder, draws, uniforms, input.cases, &mut stamps[SPAN_CASES]);
        if q != 0 {
            let written = stamps.iter().enumerate().filter(|(_, s)| s.written()).fold(0, |m, (k, _)| m | 1 << k);
            g.timestamps(encoder, q, written);
            for (k, ms) in self.gpu_spans.iter_mut().enumerate() {
                *ms = g.gpu_ms(k as u32);
            }
            self.gpu_ms = g.gpu_ms(GPU_SPANS.len() as u32);
        }
        g.submit(encoder);
        release(encoder);
        release(canvas_view);
        self.stat(draws, world, badges)
    }

    fn draw_cases(&mut self, g: &Gpu, encoder: u32, draws: &[u32], uniforms: &[f32], cases: &[(u32, u32, u32)], stamp: &mut Stamp) {
        for &(id, model, image) in cases {
            let Some(s) = surface(id) else { continue };
            if s.role != surface_role::CASE {
                continue;
            }
            // The case's actor's draw, and where it stands; nothing drawn until
            // the actor has one.
            let Some(own) = draws.as_chunks::<DRAW_WORDS>().0.iter().find(|d| d[4] == PIPELINE_SKINNED && d[2] == model) else {
                continue;
            };
            targets(g, s);
            let slot = xpute_runtime::abi::handle::handle_slot(id) as usize;
            if self.cases[slot].is_none() {
                self.cases[slot] = Some(Case::new(g, &mut self.pipelines, &self.shadow, &self.ao, self.atmosphere.entries(), self.align));
            }
            let world_frame = self.frame;
            let case = self.cases[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            case.skinned.set_skin(g, model, image);
            let at = own[3] as usize * UNIFORM_FLOATS;
            let actor = [uniforms[at] as f64, uniforms[at + 1] as f64, uniforms[at + 2] as f64];
            // The case's view, with the actor's place in the world taken back
            // out so it stands at the case's origin; the eye where the fog reads it.
            let u = &mut case.frame;
            *u = world_frame;
            let (mut proj, mut view, mut t, mut m, mut n) = (mat4(), mat4(), mat4(), mat4(), mat4());
            mat4_perspective(&mut proj, CASE_FOV_DEG.to_radians(), s.width as f64 / s.height as f64, 0.1, 100.0);
            mat4_look_at(&mut view, CASE_EYE, CASE_TARGET, [0.0, 0.0, 1.0]);
            mat4_translation(&mut t, -actor[0], -actor[1], -actor[2]);
            mat4_mul(&mut m, &GL_TO_WGPU, &proj);
            mat4_mul(&mut n, &m, &view);
            mat4_mul(&mut m, &n, &t);
            put_mat(u, VIEW_PROJ, &m);
            u[CAMERA_POS] = (CASE_EYE[0] + actor[0]) as f32;
            u[CAMERA_POS + 1] = (CASE_EYE[1] + actor[1]) as f32;
            u[CAMERA_POS + 2] = (CASE_EYE[2] + actor[2]) as f32;
            // No fog: the near and far pushed out of reach (crates/pxl/web/kernel/src/layer/environment/state.rs's
            // fog_near and fog_far in the run).
            u[ENV + 7] = 1e9;
            u[ENV + 11] = 2e9;
            u[VIEWPORT] = s.width as f32;
            u[VIEWPORT + 1] = s.height as f32;
            u[SHADOW_STRENGTH] = 0.0;
            u[VIGNETTE] = 0.0;
            u[SCREEN_AO] = 0.0;
            // No planet: the case is a room of its own.
            u[PLANET + 2] = 0.0;
            g.write_buffer(case.frame_buffer, 0, bytes(&case.frame));
            case.skinned.prepare(g, own, uniforms);
            let view_id = object();
            g.current(s.handle, view_id);
            let pass = object();
            g.pass(pass, encoder, &pass_desc(view_id, load::CLEAR, s.depth_view, load::CLEAR, 0.0, stamp.pass()));
            case.skinned.draw(g, pass, own, blocks());
            g.end(pass);
            release(pass);
            release(view_id);
        }
    }

    /// The frame's census, taken from the draw list. It is the draws and not
    /// a list of them because `draw_meshes` stops on a draw it cannot issue,
    /// so the two cannot differ.
    fn stat(&self, draws: &[u32], world: &Surface, badges: u32) -> FrameStat {
        let b = blocks();
        let mut triangles = 0u32;
        let mut census = [[0.0; 2]; PIPELINES as usize];
        for d in draws.as_chunks::<DRAW_WORDS>().0 {
            let tris = match d[4] {
                PIPELINE_CHUNK | PIPELINE_CHUNK_COARSE | PIPELINE_GHOST_BOX | PIPELINE_GHOST_BOX_COPLANAR => d[1] * 12,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, index_cnt)| index_cnt / 3),
                PIPELINE_PUCK_ZONE => 12 * d[1],
                PIPELINE_PUCK_BEACON => 8,
                PIPELINE_BADGE => 2 * d[1],
                _ => 0,
            };
            triangles += tris;
            if let Some(c) = census.get_mut(d[4] as usize) {
                c[0] += 1.0;
                c[1] += tris as f64;
            }
        }
        let px = world.width as f64 * world.height as f64;
        let _ = badges;
        FrameStat {
            gpu_ms: self.gpu_ms,
            draw_calls: (draws.len() / DRAW_WORDS) as f64,
            triangles: triangles as f64,
            buffers: b.count as f64,
            buffer_bytes: b.bytes as f64,
            depth_mb: round(px * 4.0 / MB),
            shadow_mb: round(self.shadow.bytes as f64 / MB),
            layers_mb: round(self.layers.bytes as f64 / MB),
            px,
            census,
            gpu_spans: self.gpu_spans,
            arena_mb: round(crate::gpu::arena::arena_bytes() as f64 / MB),
        }
    }
}

/// A surface's context configured, and its depth made at its buffer's size:
/// at once where there is none, and for a new size once it has held
/// SETTLE_TURNS turns — until then the buffer it has is drawn and the page
/// scales it.
fn targets(g: &Gpu, s: &mut Surface) {
    // The world's surface at what the kernel's resolution chose; a case at
    // the device's own.
    let ratio = (s.role == surface_role::WORLD).then(|| stage_lane()[STAGE_AT::RESOLUTION]).filter(|r| *r > 0.0);
    let (width, height) = s.buffer_px(ratio);
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
    for id in [s.depth_view, s.depth] {
        release(id);
    }
    s.depth = object();
    g.texture(s.depth, width, height, 1, format::DEPTH32, 1, texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING);
    s.depth_view = object();
    g.view(s.depth_view, s.depth, dimension::D2, 0, 1, 0, 1);
    s.width = width;
    s.height = height;
}

/// The turn's draw: the world's surface and every case, once a device has
/// answered and the world's canvas is here. Without either the lists are
/// recorded for the frame that has both, and nothing is drawn.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == surface_role::WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), *DEVICE.get(|| None), world) else {
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), input.lists);
        return FrameOutput { redraw: false, stat: None };
    };
    // The GPU has as many frames as it may hold: this one is skipped rather
    // than waited for, what it listed put on the device, and the next asked.
    if !frame_room() {
        take_lists(Some(g), input.lists);
        return FrameOutput { redraw: true, stat: None };
    }
    let slot = RENDERER.get(|| None);
    let r = slot.get_or_insert_with(|| Renderer::new(g, device, input.handheld));
    blocks().restore(g);
    let stat = r.frame(g, world, input);
    FrameOutput {
        redraw: *REDRAW.get(|| false),
        stat: Some(stat),
    }
}

/// The lists the scheduler left, as `row` names their lanes: the blocks it
/// named recorded, and put on the device or let go there when `g` has one;
/// and the frame's draws and their uniforms. Blocks listed with no device
/// wait in the table for the next one.
fn take_lists<'a>(g: Option<&Gpu>, lists: Lists<'a>) -> (&'a [u32], &'a [f32]) {
    let b = blocks();
    for u in lists.uploads.as_chunks::<UPLOAD_WORDS>().0 {
        b.insert(g, u[0], u[1], u[2], u[3], u[4]);
    }
    for r in lists.rewrites.as_chunks::<REWRITE_WORDS>().0 {
        b.rewrite(g, r[0], r[1], r[2]);
    }
    for &handle in lists.frees {
        b.remove(handle);
    }
    (lists.draws, lists.uniforms)
}
