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
//! buffer, the multisampled colour and depth where MSAA is on — four samples a
//! pixel, the one count WebGPU offers, the largest allocation the page makes —
//! the sun's shadow map, the texture pack's layers with their mip chains, the
//! badge sheet. Reversed float depth throughout, near at 1 and far at 0: a
//! float's precision is densest near zero and a perspective depth densest near
//! the eye, so the two cancel and the far terrain's coplanar faces stop
//! fighting.

use xpute_core::kit::mat4::{mat4, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_translation, Mat4, Vec3};
use xpute_core::sys::global::Global;
use xpute_core::sys::mem::{alloc, free};

use crate::asset::atlas::BillboardAtlas;
use crate::frame::draw::{
    DRAW_WORDS, PIPELINE_BADGE, PIPELINE_CHUNK, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE,
    PIPELINE_SKINNED, REWRITE_WORDS, UNIFORM_FLOATS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{FRAME_AT, FRAME_FLOATS, PIPELINES, PIPELINE_CHUNK_DEPTH, PIPELINE_SKY};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, forget_objects, gpu, object, pass_desc, release, texture_usage, GpuApi, NONE};
use crate::gpu::api_spec::{dimension, format, load, resource};
use crate::gpu::layers::{Layers, Mips, LAYER_TEX_SZ};
use crate::gpu::pass::badges::BadgePass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::shadow::Shadow;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, ROLE_CASE, ROLE_WORLD, SETTLE_TURNS, SURFACES};
use crate::look::environment::ENV_OTHER_LIGHT;

/// GL clip z [-1, 1] to reversed WebGPU clip z [1, 0], column-major.
pub const GL_TO_WGPU: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -0.5, 0.0, 0.0, 0.0, 0.5, 1.0];

/// The frame uniform's fields, in floats (shader.rs's FRAME_AT).
const VIEW_PROJ: usize = FRAME_AT[0] as usize;
const CAMERA_POS: usize = FRAME_AT[1] as usize;
const EDGE_STRENGTH: usize = FRAME_AT[2] as usize;
const ENV: usize = FRAME_AT[3] as usize;
const LOW_SPEC: usize = FRAME_AT[4] as usize;
const EDGE_WIDTH: usize = FRAME_AT[5] as usize;
const RIM_INTENSITY: usize = FRAME_AT[6] as usize;
const VIEWPORT: usize = FRAME_AT[7] as usize;
const SHADOW_STRENGTH: usize = FRAME_AT[8] as usize;
const SHADOW_TEXEL: usize = FRAME_AT[9] as usize;
const VIGNETTE: usize = FRAME_AT[10] as usize;
const LIGHT_VIEW_PROJ: usize = FRAME_AT[11] as usize;
const OTHER: usize = FRAME_AT[12] as usize;

/// Floats the sky uniform holds (shader/sky.wgsl's Sky).
const SKY_FLOATS: usize = 16 + 4 * 7;
/// The visibility lane past a chunk's vertex lanes, bytes a vertex, which the
/// index lane follows.
const VIS_BYTES_PER_VERTEX: u32 = 4;
/// Floats of the frame's draws storage made before the first frame needs more.
const DRAWS_INITIAL: u32 = 256 * UNIFORM_FLOATS as u32;
const MB: f64 = 1_048_576.0;

pub type Frame = [f32; FRAME_FLOATS as usize];

/// What the kernel hands a turn's draw: every input the frame's passes read
/// that is not the renderer's own.
pub struct FrameInput<'a> {
    /// The draw list's lanes, as the scheduler's `DrawList::take` gave them.
    pub lists: [usize; 10],
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
    /// The billboards' sheet, whose changes the badge texture mirrors.
    pub atlas: &'a mut BillboardAtlas,
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
    /// The targets' and textures' megabytes: depth, multisampled colour, the
    /// shadow map, the texture pack's layers.
    pub depth_mb: f64,
    pub msaa_mb: f64,
    pub shadow_mb: f64,
    pub layers_mb: f64,
    /// The world surface's width times its height.
    pub px: f64,
    /// Draws and triangles, by pipeline.
    pub census: [[f64; 2]; PIPELINES as usize],
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

/// A frame group: the frame uniform at 0 and a shadow map's two entries.
pub fn frame_group(g: &GpuApi, layout: u32, frame: u32, shadow: [u32; 10]) -> u32 {
    let id = object();
    let mut e = [0u32; 15];
    e[..5].copy_from_slice(&buffer_entry(0, frame));
    e[5..].copy_from_slice(&shadow);
    (g.bind_group)(id, layout, &e);
    id
}

/// A skin's texture and its sampler as a group, over the image `image`; none
/// while the image has not arrived. The texture, its view and the group.
pub fn skin_group(g: &GpuApi, layout: u32, sampler: u32, image: u32) -> Option<[u32; 3]> {
    let texture = object();
    if !(g.image_texture)(texture, image) {
        release(texture);
        return None;
    }
    let view = object();
    (g.view)(view, texture, dimension::D2, 0, 1, 0, 1);
    let group = object();
    (g.bind_group)(group, layout, &[0, resource::VIEW, view, 0, 0, 1, resource::SAMPLER, sampler, 0, 0]);
    Some([texture, view, group])
}

/// A block of the mesh section words are laid out in before they go to the
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
            (self.at, self.bytes) = alloc(size).expect("the heap has no room for the renderer's scratch");
        }
        // SAFETY: a block of the mesh section this scratch holds alone, at
        // least `bytes` long.
        unsafe { core::slice::from_raw_parts_mut(self.at as *mut u8, bytes as usize) }
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
/// device a copy: a block stays in its pool until its free is listed, so a
/// device that went away, or lists taken while there was none, are made good
/// from the addresses. Open addressing in lanes of the fixed section, since
/// the handles are the list's and sparse.
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
    // SAFETY: a block of a pool the draw list named, live until its free is
    // listed, in the memory addressed from 0.
    unsafe { core::slice::from_raw_parts(at as usize as *const u8, n as usize) }
}

fn fixed_lane() -> &'static mut [u32] {
    let (at, _) = alloc((BLOCK_SLOTS * 4) as u32).expect("the heap has no room for the renderer's blocks");
    // SAFETY: a block never given back, BLOCK_SLOTS words.
    let lane = unsafe { core::slice::from_raw_parts_mut(at as *mut u32, BLOCK_SLOTS) };
    lane.fill(0);
    lane
}

fn home(handle: u32) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - BLOCK_SLOTS.trailing_zeros())) as usize
}

impl Blocks {
    fn new() -> Blocks {
        Blocks {
            handle: fixed_lane(),
            at: fixed_lane(),
            len: fixed_lane(),
            vertex_cnt: fixed_lane(),
            index_cnt: fixed_lane(),
            buffer: fixed_lane(),
            group: fixed_lane(),
            count: 0,
            bytes: 0,
            missing: 0,
        }
    }

    /// The device went away with every buffer and group: the blocks stand,
    /// each to be put on the next one.
    fn lose(&mut self) {
        self.buffer.fill(0);
        self.group.fill(0);
        self.missing = self.count;
    }

    /// Puts every block that has no buffer on the device.
    fn restore(&mut self, g: &GpuApi) {
        if self.missing == 0 {
            return;
        }
        for i in 0..BLOCK_SLOTS {
            if self.handle[i] != EMPTY && self.buffer[i] == 0 {
                self.put(g, i);
            }
        }
        self.missing = 0;
    }

    /// A buffer for the block at `i`, its bytes written.
    fn put(&mut self, g: &GpuApi, i: usize) {
        let buffer = object();
        // Every block is usable every way a draw reads one: a mesh's lanes as
        // vertices and indices, a decor block's as storage.
        (g.buffer)(
            buffer,
            buffer_size(self.len[i]),
            buffer_usage::VERTEX | buffer_usage::INDEX | buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        (g.write_buffer)(buffer, 0, memory(self.at[i], self.len[i] & !3));
        self.buffer[i] = buffer;
        self.group[i] = 0;
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
    fn insert(&mut self, g: Option<&GpuApi>, handle: u32, at: u32, len: u32, vertex_cnt: u32, index_cnt: u32) {
        self.remove(handle);
        assert!((self.count as usize) < BLOCK_SLOTS / 2, "more than {} blocks listed", BLOCK_SLOTS / 2);
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
        match g {
            Some(g) => self.put(g, i),
            None => self.missing += 1,
        }
    }

    /// The block's bytes changed at `at`: written into its buffer when it has
    /// one, and read from there when it is next put on a device.
    fn rewrite(&mut self, g: Option<&GpuApi>, handle: u32, at: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.at[i] = at;
        if let (Some(g), buffer @ 1..) = (g, self.buffer[i]) {
            (g.write_buffer)(buffer, 0, memory(at, len.min(self.len[i]) & !3));
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
    pub fn group(&mut self, g: &GpuApi, handle: u32, layout: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.buffer[i] != 0)?;
        if self.group[i] == 0 {
            let id = object();
            (g.bind_group)(id, layout, &buffer_entry(0, self.buffer[i]));
            self.group[i] = id;
        }
        Some(self.group[i])
    }
}

static BLOCKS: Global<Blocks> = Global::new();

fn blocks() -> &'static mut Blocks {
    BLOCKS.get(Blocks::new)
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

/// The host's report that the device went away, and everything on it.
pub fn gpu_lost() {
    DEVICE.set(None);
    RENDERER.set(None);
    forget_objects();
    blocks().lose();
    surfaces_lost();
}

/// A surface closed: what it held on the device let go.
pub fn surface_closed(s: Surface) {
    for id in [s.depth_view, s.depth, s.msaa_view, s.msaa] {
        release(id);
    }
    let slot = xpute_core::ds::handle::handle_slot(s.handle) as usize;
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
    fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32, frame_buffer: u32) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, format::CANVAS, samples);
        let draw_buffer = object();
        (g.buffer)(draw_buffer, 16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let frame_group = object();
        (g.bind_group)(frame_group, built.layouts[0], &buffer_entry(0, frame_buffer));
        let draw_group = object();
        (g.bind_group)(draw_group, built.layouts[1], &buffer_entry(0, draw_buffer));
        DebugPass {
            built,
            draw_buffer,
            frame_group,
            draw_group,
        }
    }

    /// The frame's segments into the open pass; how many vertices.
    fn draw(&self, g: &GpuApi, pass: u32, draws: &[u32], uniforms: &[f32], blocks: &Blocks) -> u32 {
        let Some(d) = draws.chunks_exact(DRAW_WORDS).find(|d| d[4] == PIPELINE_DEBUG) else { return 0 };
        let (Some((block, _, _)), count) = (blocks.get(d[0]), d[1]) else { return 0 };
        if count == 0 {
            return 0;
        }
        let at = d[3] as usize * UNIFORM_FLOATS;
        (g.write_buffer)(self.draw_buffer, 0, bytes(&[uniforms[at], uniforms[at + 1], uniforms[at + 2], 0.0]));
        (g.set_pipeline)(pass, self.built.pipeline);
        (g.set_bind_group)(pass, 0, self.frame_group, &[]);
        (g.set_bind_group)(pass, 1, self.draw_group, &[]);
        (g.set_vertex_buffer)(pass, 0, block, 0, count * 24);
        (g.draw)(pass, count, 1, 0, 0);
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
const CASE_EYE: Vec3 = Vec3 { x: 0.0, y: -2.2, z: 0.9 };
const CASE_TARGET: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.55 };
const CASE_FOV_DEG: f64 = 35.0;

impl Case {
    fn new(g: &GpuApi, pipelines: &mut Pipelines, shadow: &Shadow, align: u32) -> Case {
        let frame_buffer = object();
        (g.buffer)(frame_buffer, FRAME_FLOATS * 4, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let skinned = SkinnedPass::new(g, pipelines, 1, frame_buffer, shadow.blank_entries(1), None, align);
        Case {
            frame_buffer,
            frame: [0.0; FRAME_FLOATS as usize],
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
    samples: u32,
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
    ghost: [Built; 2],
    draws_buffer: u32,
    draws_bytes: u32,
    bind_group: u32,
    light_group: u32,
    shadow: Shadow,
    shadows: bool,
    mips: Mips,
    layers: Layers,
    decor: DecorPass,
    skinned: SkinnedPass,
    puck: PuckPass,
    badges: BadgePass,
    debug: DebugPass,
    cases: [Option<Case>; SURFACES],
    gpu_ms: f64,
}

impl Renderer {
    fn new(g: &GpuApi, device: Device, samples: u32, handheld: bool) -> Renderer {
        let mut pipelines = Pipelines::new();
        let frame_buffer = object();
        (g.buffer)(frame_buffer, FRAME_FLOATS * 4, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let sky_buffer = object();
        (g.buffer)(sky_buffer, (SKY_FLOATS * 4) as u32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let shadow = Shadow::new(g, handheld);
        let mips = Mips::new(g, &mut pipelines);
        let mut fill = Scratch::default();
        let layers = Layers::new(g, &mips, fill.take(LAYER_TEX_SZ * LAYER_TEX_SZ * 4));
        fill.free();
        let chunk = pipelines.build(g, PIPELINE_CHUNK, format::CANVAS, samples);
        let sky_built = pipelines.build(g, PIPELINE_SKY, format::CANVAS, samples);
        let sky_group = object();
        let mut sky_entries = [0u32; 10];
        sky_entries[..5].copy_from_slice(&buffer_entry(0, frame_buffer));
        sky_entries[5..].copy_from_slice(&buffer_entry(1, sky_buffer));
        (g.bind_group)(sky_group, sky_built.layouts[0], &sky_entries);
        let draws_buffer = object();
        (g.buffer)(draws_buffer, DRAWS_INITIAL * 4, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        let query_set = if device.timestamps {
            let q = object();
            (g.query_set)(q, 2);
            q
        } else {
            0
        };
        let mut r = Renderer {
            align: device.align,
            samples,
            query_set,
            chunk,
            chunk_depth: pipelines.build(g, PIPELINE_CHUNK_DEPTH, format::CANVAS, samples),
            ghost: [
                pipelines.build(g, PIPELINE_GHOST_BOX, format::CANVAS, samples),
                pipelines.build(g, PIPELINE_GHOST_BOX_COPLANAR, format::CANVAS, samples),
            ],
            decor: DecorPass::new(g, &mut pipelines, samples, frame_buffer, &shadow, device.align),
            skinned: SkinnedPass::new(
                g,
                &mut pipelines,
                samples,
                frame_buffer,
                shadow.entries(1),
                Some((shadow.light_frame, shadow.blank_entries(1))),
                device.align,
            ),
            puck: PuckPass::new(g, &mut pipelines, samples, frame_buffer),
            badges: BadgePass::new(g, &mut pipelines, samples),
            debug: DebugPass::new(g, &mut pipelines, samples, frame_buffer),
            pipelines,
            frame_buffer,
            frame: [0.0; FRAME_FLOATS as usize],
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
            mips,
            layers,
            cases: [const { None }; SURFACES],
            gpu_ms: 0.0,
        };
        r.bind_group = r.chunk_group(g, r.frame_buffer, r.shadow.entries(1));
        r.light_group = r.chunk_group(g, r.shadow.light_frame, r.shadow.blank_entries(1));
        r
    }

    /// The chunk pass's group over `frame`: the frame, a shadow map, the draws,
    /// the texture pack's two arrays and their sampler.
    fn chunk_group(&self, g: &GpuApi, frame: u32, shadow: [u32; 10]) -> u32 {
        let mut e = [0u32; 35];
        e[..5].copy_from_slice(&buffer_entry(0, frame));
        e[5..15].copy_from_slice(&shadow);
        e[15..20].copy_from_slice(&buffer_entry(3, self.draws_buffer));
        e[20..25].copy_from_slice(&[4, resource::VIEW, self.layers.top_view, 0, 0]);
        e[25..30].copy_from_slice(&[5, resource::VIEW, self.layers.side_view, 0, 0]);
        e[30..35].copy_from_slice(&[6, resource::SAMPLER, self.layers.sampler, 0, 0]);
        let id = object();
        (g.bind_group)(id, self.chunk.layouts[0], &e);
        id
    }

    /// MSAA is every pipeline's sample count and the targets': a change
    /// rebuilds what depends on it and keeps the rest.
    fn set_samples(&mut self, g: &GpuApi, samples: u32) {
        self.samples = samples;
        let p = &mut self.pipelines;
        self.chunk = p.build(g, PIPELINE_CHUNK, format::CANVAS, samples);
        self.chunk_depth = p.build(g, PIPELINE_CHUNK_DEPTH, format::CANVAS, samples);
        self.ghost = [
            p.build(g, PIPELINE_GHOST_BOX, format::CANVAS, samples),
            p.build(g, PIPELINE_GHOST_BOX_COPLANAR, format::CANVAS, samples),
        ];
        self.sky_built = p.build(g, PIPELINE_SKY, format::CANVAS, samples);
        self.decor.set_samples(g, p, samples);
        self.skinned.set_samples(g, p, samples);
        self.puck.set_samples(g, p, samples);
        self.badges.set_samples(g, p, samples);
        self.debug.built = p.build(g, PIPELINE_DEBUG, format::CANVAS, samples);
    }

    /// The frame's uniforms and the sky's, laid out from the stage and the
    /// environment `env` for the world's surface at (width, height) device
    /// pixels.
    fn lay_out(&mut self, g: &GpuApi, width: u32, height: u32, env: &[f32]) {
        let s = stage_lane();
        let (mut a, mut m, mut proj) = (mat4(), mat4(), mat4());
        a.copy_from_slice(&s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16]);
        mat4_mul(&mut proj, &GL_TO_WGPU, &a);
        // The badges take the view and the projection apart: their size is
        // measured in view space.
        let b = &mut self.badges;
        put_mat(&mut b.view, 0, s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].try_into().expect("16 doubles"));
        put_mat(&mut b.proj, 0, &proj);
        b.px_k = (s[STAGE_AT::VIEWPORT + 1] / (2.0 * (s[STAGE_AT::FOV] * core::f64::consts::PI / 360.0).tan())) as f32;
        b.fade_start = s[STAGE_AT::FADE_START] as f32;
        b.fade_end = s[STAGE_AT::FADE_END] as f32;
        b.near = s[STAGE_AT::NEAR] as f32;
        b.far = s[STAGE_AT::FAR] as f32;
        b.cell = s[STAGE_AT::LOD_CELL] as f32;

        let u = &mut self.frame;
        a.copy_from_slice(&s[STAGE_AT::LOCAL_VIEW_PROJ..STAGE_AT::LOCAL_VIEW_PROJ + 16]);
        mat4_mul(&mut m, &GL_TO_WGPU, &a);
        put_mat(u, VIEW_PROJ, &m);
        // The camera's position in the root's frame, where the meshes are.
        for i in 0..3 {
            u[CAMERA_POS + i] = s[STAGE_AT::EYE_LOCAL + i] as f32;
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
        // The sun's shadow, over the map centre — the root frame's origin undone
        // — from the light the environment lit the terrain with.
        self.shadows = s[STAGE_AT::SHADOWS] != 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_TEXEL] = self.shadow.texel as f32;
        if self.shadows {
            let target = Vec3 {
                x: -s[STAGE_AT::TRANSLATION],
                y: -s[STAGE_AT::TRANSLATION + 1],
                z: -s[STAGE_AT::TRANSLATION + 2],
            };
            self.shadow.aim(g, &env[0..3], target);
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
        }

        // The sky, in the view frame, where the dome stood.
        a.copy_from_slice(&s[STAGE_AT::VIEW_MATRIX..STAGE_AT::VIEW_MATRIX + 16]);
        mat4_mul(&mut m, &proj, &a);
        mat4_invert(&mut a, &m);
        put_mat(&mut self.sky, 0, &a);
        for i in 0..3 {
            self.sky[16 + i] = s[STAGE_AT::EYE + i] as f32;
        }
        self.sky[19] = s[STAGE_AT::SKY_RADIUS] as f32;
        self.sky[20..44].copy_from_slice(&env[24..48]);
    }

    /// Every chunk-pipeline draw of pipeline `which`; with `own`, that pipeline
    /// set before the first. Whether any was drawn.
    fn draw_meshes(&self, g: &GpuApi, pass: u32, draws: &[u32], which: u32, own: Option<u32>) -> bool {
        let b = blocks();
        let vertex_bytes = self.chunk.vertex_bytes() + VIS_BYTES_PER_VERTEX;
        let mut any = false;
        for d in draws.chunks_exact(DRAW_WORDS) {
            let (handle, vertex_cnt, index_cnt, uniform, pipeline) = (d[0], d[1], d[2], d[3], d[4]);
            if pipeline != which {
                continue;
            }
            let Some((buffer, _, _)) = b.get(handle) else { continue };
            if let (Some(p), false) = (own, any) {
                (g.set_pipeline)(pass, p);
                (g.set_bind_group)(pass, 0, self.bind_group, &[]);
            }
            any = true;
            let mut offset = 0;
            for lane in 0..self.chunk.lanes {
                (g.set_vertex_buffer)(pass, lane as u32, buffer, offset, 0);
                offset += vertex_cnt * self.chunk.strides[lane];
            }
            (g.set_index_buffer)(pass, buffer, vertex_cnt * vertex_bytes, index_cnt * 4);
            (g.draw_indexed)(pass, index_cnt, 1, 0, uniform);
        }
        any
    }

    /// The world, on the world's surface, and every case on its own; what the
    /// frame cost.
    fn frame(&mut self, g: &GpuApi, world: &mut Surface, input: FrameInput) -> FrameStat {
        self.decor.set_models(input.decor);
        for &(model, image) in input.skins {
            self.skinned.set_skin(g, model, image);
        }
        self.layers.step(g, &self.mips, input.sheet);
        self.badges.sync_sheet(g, &self.mips, input.atlas);
        let (draws, uniforms) = take_lists(Some(g), input.lists);

        let samples = self.samples;
        targets(g, world, samples);
        self.lay_out(g, world.width, world.height, input.environment);
        (g.write_buffer)(self.frame_buffer, 0, bytes(&self.frame));
        (g.write_buffer)(self.sky_buffer, 0, bytes(&self.sky));
        let uniform_bytes = (uniforms.len() * 4) as u32;
        if uniform_bytes > self.draws_bytes {
            release(self.bind_group);
            release(self.light_group);
            release(self.draws_buffer);
            self.draws_bytes = uniform_bytes * 2;
            self.draws_buffer = object();
            (g.buffer)(self.draws_buffer, self.draws_bytes, buffer_usage::STORAGE | buffer_usage::COPY_DST);
            self.bind_group = self.chunk_group(g, self.frame_buffer, self.shadow.entries(1));
            self.light_group = self.chunk_group(g, self.shadow.light_frame, self.shadow.blank_entries(1));
        }
        if !uniforms.is_empty() {
            (g.write_buffer)(self.draws_buffer, 0, bytes(uniforms));
        }
        self.decor.prepare(g, draws, uniforms);
        self.skinned.prepare(g, draws, uniforms);

        let b = blocks();
        let encoder = object();
        (g.encoder)(encoder);
        // The sun's shadow map first, from the frame's lists, where it is on.
        if self.shadows {
            let pass = self.shadow.begin(g, encoder);
            (g.set_pipeline)(pass, self.chunk_depth.pipeline);
            (g.set_bind_group)(pass, 0, self.light_group, &[]);
            self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, None);
            self.decor.draw_depth(g, pass, draws, b);
            self.skinned.draw_depth(g, pass, draws, b);
            (g.end)(pass);
            release(pass);
        }
        let canvas_view = object();
        (g.current)(world.handle, canvas_view);
        let (color, resolve) = if world.msaa_view != 0 { (world.msaa_view, canvas_view) } else { (canvas_view, 0) };
        let q = self.query_set;
        let pass = object();
        (g.pass)(
            pass,
            encoder,
            &pass_desc(color, resolve, load::CLEAR, world.depth_view, load::CLEAR, 0.0, q, if q != 0 { 0 } else { NONE }, NONE),
        );
        (g.set_pipeline)(pass, self.sky_built.pipeline);
        (g.set_bind_group)(pass, 0, self.sky_group, &[]);
        (g.draw)(pass, 3, 1, 0, 0);
        (g.set_pipeline)(pass, self.chunk.pipeline);
        (g.set_bind_group)(pass, 0, self.bind_group, &[]);
        self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, None);
        self.decor.draw(g, pass, draws, b);
        self.skinned.draw(g, pass, draws, b);
        self.puck.draw_beacon(g, pass, draws, uniforms);
        self.puck.draw_zone(g, pass, draws, uniforms, b);
        // What is blended goes last, over everything opaque.
        self.draw_meshes(g, pass, draws, PIPELINE_GHOST_BOX, Some(self.ghost[0].pipeline));
        self.draw_meshes(g, pass, draws, PIPELINE_GHOST_BOX_COPLANAR, Some(self.ghost[1].pipeline));
        self.decor.draw_ghosts(g, pass, draws, b);
        self.debug.draw(g, pass, draws, uniforms, b);
        (g.end)(pass);
        release(pass);
        let badges = self.badges.draw(g, encoder, color, resolve, world.depth_view, [world.width, world.height], draws, uniforms, b);
        self.draw_cases(g, encoder, draws, uniforms, input.cases);
        if q != 0 {
            // The frame's end, on a pass of its own that draws nothing, so the
            // two timestamps bracket every pass of the frame.
            let end = object();
            (g.pass)(end, encoder, &pass_desc(color, resolve, load::LOAD, 0, load::CLEAR, 0.0, q, NONE, 1));
            (g.end)(end);
            release(end);
            (g.timestamps)(encoder, q);
            self.gpu_ms = (g.gpu_ms)();
        }
        (g.submit)(encoder);
        release(encoder);
        release(canvas_view);
        self.stat(draws, world, badges)
    }

    fn draw_cases(&mut self, g: &GpuApi, encoder: u32, draws: &[u32], uniforms: &[f32], cases: &[(u32, u32, u32)]) {
        for &(id, model, image) in cases {
            let Some(s) = surface(id) else { continue };
            if s.role != ROLE_CASE {
                continue;
            }
            // The case's actor's draw, and where it stands; nothing drawn until
            // the actor has one.
            let Some(own) = draws.chunks_exact(DRAW_WORDS).find(|d| d[4] == PIPELINE_SKINNED && d[2] == model) else {
                continue;
            };
            targets(g, s, 1);
            let slot = xpute_core::ds::handle::handle_slot(id) as usize;
            if self.cases[slot].is_none() {
                self.cases[slot] = Some(Case::new(g, &mut self.pipelines, &self.shadow, self.align));
            }
            let world_frame = self.frame;
            let case = self.cases[slot].as_mut().expect("just made");
            case.skinned.set_skin(g, model, image);
            let at = own[3] as usize * UNIFORM_FLOATS;
            let actor = [uniforms[at] as f64, uniforms[at + 1] as f64, uniforms[at + 2] as f64];
            // The case's view, with the actor's place in the world taken back
            // out so it stands at the case's origin; the eye where the fog reads it.
            let u = &mut case.frame;
            *u = world_frame;
            let (mut proj, mut view, mut t, mut m, mut n) = (mat4(), mat4(), mat4(), mat4(), mat4());
            mat4_perspective(&mut proj, CASE_FOV_DEG.to_radians(), s.width as f64 / s.height as f64, 0.1, 100.0);
            mat4_look_at(&mut view, CASE_EYE, CASE_TARGET, Vec3 { x: 0.0, y: 0.0, z: 1.0 });
            mat4_translation(&mut t, -actor[0], -actor[1], -actor[2]);
            mat4_mul(&mut m, &GL_TO_WGPU, &proj);
            mat4_mul(&mut n, &m, &view);
            mat4_mul(&mut m, &n, &t);
            put_mat(u, VIEW_PROJ, &m);
            u[CAMERA_POS] = (CASE_EYE.x + actor[0]) as f32;
            u[CAMERA_POS + 1] = (CASE_EYE.y + actor[1]) as f32;
            u[CAMERA_POS + 2] = (CASE_EYE.z + actor[2]) as f32;
            // No fog: the near and far pushed out of reach (crates/pxl/kernel/src/layer/environment/state.rs's
            // fog_near and fog_far in the run).
            u[ENV + 7] = 1e9;
            u[ENV + 11] = 2e9;
            u[VIEWPORT] = s.width as f32;
            u[VIEWPORT + 1] = s.height as f32;
            u[SHADOW_STRENGTH] = 0.0;
            u[VIGNETTE] = 0.0;
            (g.write_buffer)(case.frame_buffer, 0, bytes(&case.frame));
            case.skinned.prepare(g, own, uniforms);
            let view_id = object();
            (g.current)(s.handle, view_id);
            let pass = object();
            (g.pass)(pass, encoder, &pass_desc(view_id, 0, load::CLEAR, s.depth_view, load::CLEAR, 0.0, 0, NONE, NONE));
            case.skinned.draw(g, pass, own, blocks());
            (g.end)(pass);
            release(pass);
            release(view_id);
        }
    }

    fn stat(&self, draws: &[u32], world: &Surface, badges: u32) -> FrameStat {
        let b = blocks();
        let mut triangles = 0u32;
        let mut census = [[0.0; 2]; PIPELINES as usize];
        for d in draws.chunks_exact(DRAW_WORDS) {
            let tris = match d[4] {
                PIPELINE_CHUNK | PIPELINE_GHOST_BOX | PIPELINE_GHOST_BOX_COPLANAR => d[2] / 3,
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
            depth_mb: (px * 4.0 * world.samples as f64 / MB).round(),
            msaa_mb: if world.samples > 1 { (px * 4.0 * world.samples as f64 / MB).round() } else { 0.0 },
            shadow_mb: (self.shadow.bytes as f64 / MB).round(),
            layers_mb: (self.layers.bytes as f64 / MB).round(),
            px,
            census,
        }
    }
}

/// A surface's context configured, and its depth and multisampled colour made
/// at its buffer's size and `samples`: at once where there are none or the
/// samples changed, and for a new size once it has held SETTLE_TURNS turns —
/// until then the buffer it has is drawn and the page scales it.
fn targets(g: &GpuApi, s: &mut Surface, samples: u32) {
    let (width, height) = s.buffer_px();
    if !s.configured {
        (g.configure)(s.handle);
        s.configured = true;
        s.depth = 0;
    }
    let made = s.depth != 0 && s.samples == samples;
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
    (g.resize)(s.handle, width, height);
    for id in [s.depth_view, s.depth, s.msaa_view, s.msaa] {
        release(id);
    }
    s.depth = object();
    (g.texture)(
        s.depth,
        width,
        height,
        1,
        format::DEPTH32,
        1,
        samples,
        texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
    );
    s.depth_view = object();
    (g.view)(s.depth_view, s.depth, dimension::D2, 0, 1, 0, 1);
    if samples > 1 {
        s.msaa = object();
        (g.texture)(s.msaa, width, height, 1, format::CANVAS, 1, samples, texture_usage::RENDER_ATTACHMENT);
        s.msaa_view = object();
        (g.view)(s.msaa_view, s.msaa, dimension::D2, 0, 1, 0, 1);
    } else {
        s.msaa = 0;
        s.msaa_view = 0;
    }
    s.width = width;
    s.height = height;
    s.samples = samples;
}

/// The turn's draw: the world's surface and every case, once a device has
/// answered and the world's canvas is here. Without either the lists are
/// recorded for the frame that has both, and nothing is drawn.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == ROLE_WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), *DEVICE.get(|| None), world) else {
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), input.lists);
        return FrameOutput { redraw: false, stat: None };
    };
    let samples = if stage_lane()[STAGE_AT::MSAA] != 0.0 { 4 } else { 1 };
    let slot = RENDERER.get(|| None);
    let r = slot.get_or_insert_with(|| Renderer::new(g, device, samples, input.handheld));
    blocks().restore(g);
    if r.samples != samples {
        r.set_samples(g, samples);
    }
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
fn take_lists(g: Option<&GpuApi>, row: [usize; 10]) -> (&'static [u32], &'static [f32]) {
    // SAFETY: the list's lanes, in the memory addressed from 0, valid until
    // the next take.
    let words = |at: usize, n: usize| unsafe { core::slice::from_raw_parts(at as *const u32, n) };
    let b = blocks();
    for u in words(row[0], row[1] * UPLOAD_WORDS).chunks_exact(UPLOAD_WORDS) {
        b.insert(g, u[0], u[1], u[2], u[3], u[4]);
    }
    for r in words(row[8], row[9] * REWRITE_WORDS).chunks_exact(REWRITE_WORDS) {
        b.rewrite(g, r[0], r[1], r[2]);
    }
    for &handle in words(row[2], row[3]) {
        b.remove(handle);
    }
    // SAFETY: as above; the uniforms' floats are 32 bits, 8 a draw.
    let uniforms = unsafe { core::slice::from_raw_parts(row[6] as *const f32, row[7] * UNIFORM_FLOATS) };
    (words(row[4], row[5] * DRAW_WORDS), uniforms)
}
