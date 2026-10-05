// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn.
//! Depth is reversed f32 throughout (depth.wgsl).

use pxl_spec::space::mat4::{mat4, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_translation, Mat4};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::v3::{norm3, sub3, V3};
use xpute_core::math::scalar::round;
use xpute_runtime::abi::handle::handle_slot;
use xpute_runtime::global::Global;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

use crate::asset::atlas::BillboardAtlas;
use crate::frame::draw::{
    uniform_of, Lists, DRAW_WORDS, PIPELINE_BADGE, PIPELINE_CHUNK, PIPELINE_CHUNK_COARSE, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS, UNIFORM_FLOATS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{FRAME_AT, FRAME_FLOATS, PIPELINES, PIPELINE_CHUNK_COARSE_DEPTH, PIPELINE_CHUNK_COARSE_PREPASS, PIPELINE_CHUNK_DEPTH, PIPELINE_CHUNK_PREPASS, PIPELINE_SKY};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, forget_objects, frame_begin, frame_commit, frame_room, gpu, object, pass_desc, release, room, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{dimension, format, load, resource, surface_role};
use crate::gpu::arena::{arena_buffer, arena_bytes, arena_lose};
use crate::gpu::atmosphere::{Atmosphere, AIR_WORDS};
use crate::gpu::clouds::{self, CloudPass};
use crate::gpu::image_map::ImageMap;
use crate::gpu::layers::{Layers, Mips};
use crate::gpu::pass::badges::BadgePass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::model::{frame_layout, FrameGroups, Stage};
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::shadow::Shadow;
use crate::gpu::stars::StarPass;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::look::environment::{ENV_AIR, ENV_FOG_FAR, ENV_FOG_NEAR, ENV_OTHER_LIGHT, ENV_STARS};
use crate::look::globe::{Globe, MERCATOR_RADIUS_U};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// GL clip z [-1, 1] to reversed WebGPU clip z [1, 0], column-major.
pub const GL_TO_WGPU: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -0.5, 0.0, 0.0, 0.0, 0.5, 1.0];

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
const UP: usize = FRAME_AT[17];

/// The ray, the celestial pole, and whether the Moon's face has arrived.
pub(crate) const SKY_FLOATS: usize = 16;
pub(crate) const SKY_POLE: usize = 12;
pub(crate) const SKY_MAPPED: usize = 15;
const BOX_VERTICES: u32 = 36;
const DRAWS_INITIAL: u32 = 256 * UNIFORM_FLOATS as u32;
const MB: f64 = 1_048_576.0;
/// The Moon's near side as download_moon cuts it; sky.wgsl reads its half.
const MOON_TEX_SZ: u32 = 512;

/// In the order the frame encodes them; the profiler's GPU_SPAN_NAMES match.
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

pub struct FrameInput<'a> {
    pub lists: Lists<'a>,
    pub environment: &'a [f32],
    /// 0 while it loads.
    pub decor: &'a [(u32, Option<u32>, Extent)],
    pub skins: &'a [(u32, u32)],
    pub cases: &'a [(u32, u32, u32)],
    pub sheet: fn() -> Option<u32>,
    pub moon: fn() -> Option<u32>,
    pub earth: fn() -> Option<u32>,
    pub earth_pages: EarthPages<'a>,
    pub atlas: &'a mut BillboardAtlas,
    pub text: TextOf,
    /// A phone gets a smaller shadow map.
    pub handheld: bool,
}

/// The Earth layer's pages as the GPU reads them: the floor's window, its
/// south-west page first, and per page its layer plus one; `upload` hands each
/// page that has come to the writer while the writer takes it.
pub struct EarthPages<'a> {
    pub window: [i32; 2],
    pub layers: &'a [u32],
    pub upload: &'a mut Upload,
}

/// Hands each waiting page's layer and texels to the writer while it takes them.
pub type Upload = dyn FnMut(&mut dyn FnMut(u32, &[u8]) -> bool);

pub struct FrameOutput {
    /// A surface waits on a size to settle, so another turn is wanted.
    pub redraw: bool,
    pub stat: Option<FrameStat>,
}

#[derive(Clone, Copy)]
pub struct FrameStat {
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub buffers: f64,
    pub buffer_bytes: f64,
    pub depth_mb: f64,
    pub shadow_mb: f64,
    pub layers_mb: f64,
    pub px: f64,
    pub census: [[f64; 2]; PIPELINES as usize],
    pub gpu_spans: [f64; GPU_SPANS.len()],
    pub arena_mb: f64,
    /// 1 where the frame did not fit the GPU stream and was not drawn.
    pub skipped: f64,
}

pub fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// The planet's and the globe's four floats each into a frame.
pub(crate) fn put_globe(frame: &mut [f32], globe: &[f32; 8]) {
    frame[PLANET..PLANET + 4].copy_from_slice(&globe[..4]);
    frame[GLOBE..GLOBE + 4].copy_from_slice(&globe[4..]);
}

pub(crate) fn put_mat(out: &mut [f32], at: usize, m: &Mat4) {
    for (i, v) in m.iter().enumerate() {
        out[at + i] = *v as f32;
    }
}

/// The ray under NDC (x, y) is `x·col0 + y·col1 + col2`, taken in f64. Un-
/// projecting two points in the shader kept a direction to only ~1e-3 rad,
/// which the secant carried out to kilometers at the horizon: the clouds shook.
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
    for (k, (to, from)) in [(point(1.0, 0.0), c), (point(0.0, 1.0), c), (c, *eye)].into_iter().enumerate() {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3].iter_mut().zip(sub3(&to, &from)) {
            *o = e as f32;
        }
    }
}

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

/// Each skin image as one texture's view, made the first time a skin binds it:
/// the host lets a decoded image go as it uploads it (`Gpu::image_texture`), so
/// a skin made again, or by another pass, binds what is here. As many as the
/// skins the asset table names.
static SKIN_VIEWS: Global<Vec<(u32, u32)>> = Global::new();

fn skin_view(g: &Gpu, image: u32) -> u32 {
    let views = SKIN_VIEWS.get(Vec::new);
    if let Some(&(_, view)) = views.iter().find(|&&(i, _)| i == image) {
        return view;
    }
    let (texture, view) = (object(), object());
    g.image_texture(texture, image);
    g.view(view, texture, dimension::D2, 0, 1, 0, 1);
    views.push((image, view));
    view
}

pub fn skin_group(g: &Gpu, layout: u32, sampler: u32, image: u32) -> u32 {
    let group = object();
    g.bind_group(group, layout, &[0, resource::VIEW, skin_view(g, image), 0, 0, 1, resource::SAMPLER, sampler, 0, 0]);
    group
}

/// Records sorted by pipeline with a stable counting sort, so a pass walks
/// only its own records.
pub struct Draws<'a> {
    records: &'a [[u32; DRAW_WORDS]],
    order: &'a [u32],
    starts: [u32; PIPELINES as usize + 1],
}

impl<'a> Draws<'a> {
    fn sort(records: &'a [[u32; DRAW_WORDS]], order: &'a mut Vec<u32>) -> Draws<'a> {
        let mut starts = [0u32; PIPELINES as usize + 1];
        for d in records {
            if let Some(n) = starts.get_mut(d[4] as usize + 1) {
                *n += 1;
            }
        }
        for p in 0..PIPELINES as usize {
            starts[p + 1] += starts[p];
        }
        order.clear();
        order.resize(starts[PIPELINES as usize] as usize, 0);
        let mut next = starts;
        for (i, d) in records.iter().enumerate() {
            if let Some(at) = next.get_mut(d[4] as usize).filter(|_| d[4] < PIPELINES) {
                order[*at as usize] = i as u32;
                *at += 1;
            }
        }
        Draws { records, order, starts }
    }

    pub fn of(&self, pipeline: u32) -> impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone + 'a {
        let p = (pipeline as usize).min(PIPELINES as usize);
        let end = if p < PIPELINES as usize { self.starts[p + 1] } else { self.starts[p] };
        let records = self.records;
        self.order[self.starts[p] as usize..end as usize].iter().map(move |&i| &records[i as usize])
    }
}

/// Grown to the next power of two when a frame needs more, never shrunk.
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
        // SAFETY: a heap block this scratch holds alone, at least `bytes` long.
        unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(self.at), bytes as usize) }
    }

    pub fn free(&mut self) {
        if self.bytes > 0 {
            // SAFETY: the block this scratch held alone.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
        }
    }
}

const EMPTY: u32 = 0;

/// A power of two so a position wraps by masking, at least twice `live` so
/// open addressing never fills past half.
pub const fn block_slots(live: usize) -> usize {
    (2 * live).next_power_of_two()
}

/// Blocks by the list's handle: open addressing in `.fixed.blocks`. The heap
/// is the source and the device a copy, so a lost device is restored from the
/// addresses.
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
    missing: u32,
}

pub(crate) fn buffer_size(len: u32) -> u32 {
    ((len + 3) & !3).max(4)
}

fn memory(at: u32, n: u32) -> &'static [u8] {
    // SAFETY: a heap block the draw list named, live until its free is listed.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at as usize), n as usize) }
}

/// Handle, address, length, the two counts, buffer, group.
const BLOCK_LANES: usize = 7;

pub const fn blocks_bytes(slots: usize) -> u32 {
    (BLOCK_LANES * slots * 4) as u32
}

/// # Safety
/// `at` must address `blocks_bytes(slots)`, 4-aligned and zeroed, the
/// renderer's alone for as long as the module runs.
pub unsafe fn set_blocks(at: u32, slots: usize) {
    xpute_core::ensure!(slots.is_power_of_two(), EINVAL, slots);
    // SAFETY: the caller's range, one lane of `slots` words after another.
    let lane = |k: usize| unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize + k * slots * 4), slots) };
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

fn home(handle: u32, slots: usize) -> usize {
    (handle.wrapping_mul(0x9e37_79b1) >> (32 - slots.trailing_zeros())) as usize
}

impl Blocks {
    fn lose(&mut self) {
        self.buffer.fill(0);
        self.group.fill(0);
        self.missing = self.count;
    }

    /// As far as the turn's stream has room.
    fn restore(&mut self, g: &Gpu) {
        if self.missing == 0 {
            return;
        }
        for i in 0..self.handle.len() {
            if self.handle[i] != EMPTY && self.buffer[i] == 0 && !self.put(g, i) {
                return;
            }
        }
    }

    /// False, the block still missing, where the turn's stream has no room.
    fn put(&mut self, g: &Gpu, i: usize) -> bool {
        if !room(self.len[i]) {
            return false;
        }
        let buffer = object();
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
        let mut i = home(handle, self.handle.len());
        loop {
            match self.handle[i] {
                EMPTY => return None,
                h if h == handle => return Some(i),
                _ => i = (i + 1) & (self.handle.len() - 1),
            }
        }
    }

    /// None until the block is on the device.
    pub fn get(&self, handle: u32) -> Option<(u32, u32, u32)> {
        self.find(handle).filter(|&i| self.buffer[i] != 0).map(|i| (self.buffer[i], self.vertex_cnt[i], self.index_cnt[i]))
    }

    fn insert(&mut self, g: Option<&Gpu>, handle: u32, at: u32, len: u32, vertex_cnt: u32, index_cnt: u32) {
        self.remove(handle);
        // Slots are twice the most blocks the owners can hold, so this is that bound broken.
        xpute_core::ensure!((self.count as usize) < self.handle.len() / 2, ENOTRECOVERABLE, self.handle.len() / 2);
        let mut i = home(handle, self.handle.len());
        while self.handle[i] != EMPTY {
            i = (i + 1) & (self.handle.len() - 1);
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

    /// Where the turn's stream has no room, the buffer goes and the block is put
    /// again whole once there is.
    fn rewrite(&mut self, g: Option<&Gpu>, handle: u32, at: u32, from: u32, len: u32) {
        let Some(i) = self.find(handle) else { return };
        self.at[i] = at;
        let (Some(g), buffer @ 1..) = (g, self.buffer[i]) else { return };
        let bytes = len.min(self.len[i].saturating_sub(from)) & !3;
        if room(bytes) {
            g.write_buffer(buffer, from, memory(at + from, bytes));
        } else {
            release(self.group[i]);
            release(buffer);
            (self.buffer[i], self.group[i]) = (0, 0);
            self.missing += 1;
        }
    }

    fn remove(&mut self, handle: u32) {
        let Some(mut i) = self.find(handle) else { return };
        release(self.group[i]);
        if self.buffer[i] == 0 {
            self.missing = self.missing.saturating_sub(1);
        }
        release(self.buffer[i]);
        self.count -= 1;
        self.bytes -= buffer_size(self.len[i]) as u64;
        // Backward shift: an entry past the hole moves into it unless its home lies
        // between the hole and it, so no probe ever stops early.
        loop {
            let mut j = i;
            loop {
                j = (j + 1) & (self.handle.len() - 1);
                if self.handle[j] == EMPTY {
                    self.handle[i] = EMPTY;
                    return;
                }
                let k = home(self.handle[j], self.handle.len());
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

    /// For a draw binding it as a vertex buffer: the ghost's box goes through the chunk pipeline.
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

static REDRAW: Global<bool> = Global::new();

/// WebGPU's default minUniformBufferOffsetAlignment: an adapter's limit is a
/// power of two no larger, so a larger one is no device's.
pub const UNIFORM_ALIGN_MAX: u32 = 256;

pub fn gpu_created(align: u32, timestamps: bool) {
    DEVICE.set(Some(Device { align, timestamps }));
}

pub fn device_ready() -> bool {
    DEVICE.get(|| None).is_some()
}

pub fn gpu_lost() {
    DEVICE.set(None);
    RENDERER.set(None);
    SKIN_VIEWS.set(Vec::new());
    forget_objects();
    blocks().lose();
    arena_lose();
    surfaces_lost();
}

pub fn surface_closed(s: Surface) {
    for id in [s.depth_view, s.depth] {
        release(id);
    }
    let slot = handle_slot(s.handle) as usize;
    if let Some(r) = RENDERER.get(|| None).as_mut() {
        if let Some(case) = r.cases[slot].take() {
            case.release();
        }
    }
}

/// Each set of lines takes its own translation, so a buffer and a group a set,
/// grown to the most sets a frame has drawn.
struct DebugPass {
    built: Built,
    frame_group: u32,
    sets: Vec<(u32, u32)>,
}

impl DebugPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, format::CANVAS);
        let frame_group = object();
        g.bind_group(frame_group, built.layouts[0], &buffer_entry(0, frame_buffer));
        DebugPass { built, frame_group, sets: Vec::new() }
    }

    /// A set for each set of lines, made here so drawing makes nothing that
    /// outlives its frame.
    fn prepare(&mut self, g: &Gpu, draws: &Draws) {
        while self.sets.len() < draws.of(PIPELINE_DEBUG).len() {
            let buffer = object();
            g.buffer(buffer, 16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            let group = object();
            g.bind_group(group, self.built.layouts[1], &buffer_entry(0, buffer));
            self.sets.push((buffer, group));
        }
    }

    fn draw(&self, g: &Gpu, pass: u32, draws: &Draws, uniforms: &[f32], blocks: &Blocks) {
        let mut bound = false;
        for (k, d) in draws.of(PIPELINE_DEBUG).enumerate() {
            let (Some((block, _, _)), count) = (blocks.get(d[0]), d[1]) else { continue };
            if count == 0 {
                continue;
            }
            let (buffer, group) = self.sets[k];
            let u = uniform_of(uniforms, d);
            g.write_buffer(buffer, 0, bytes(&[u[0], u[1], u[2], 0.0]));
            if !bound {
                g.set_pipeline(pass, self.built.pipeline);
                g.set_bind_group(pass, 0, self.frame_group, &[]);
                bound = true;
            }
            g.set_bind_group(pass, 1, group, &[]);
            g.set_vertex_buffer(pass, 0, block, 0, count * 28);
            // A segment an instance: its two vertices, read as one.
            g.draw(pass, 6, count / 2, 0, 0);
        }
    }
}

/// A variant picker case: its own frame, drawn at one sample with no shadow.
struct Case {
    frame_buffer: u32,
    frame: Frame,
    frames: FrameGroups,
    skinned: SkinnedPass,
}

const CASE_EYE: V3 = [0.0, -2.2, 0.9];
const CASE_TARGET: V3 = [0.0, 0.0, 0.55];
const CASE_FOV_DEG: f64 = 35.0;

impl Case {
    fn new(g: &Gpu, pipelines: &mut Pipelines, shadow: &Shadow, ao: &ScreenAo, air: [u32; AIR_WORDS], align: u32) -> Case {
        let frame_buffer = object();
        g.buffer(frame_buffer, word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let layout = frame_layout(g, pipelines);
        Case {
            frame_buffer,
            frame: [0.0; FRAME_FLOATS],
            frames: FrameGroups::new(g, layout, frame_buffer, shadow.blank_entries(1), None, ao.blank_entry(7), air),
            skinned: SkinnedPass::new(g, pipelines, align),
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
    chunk_coarse: Built,
    chunk_coarse_depth: Built,
    chunk_coarse_prepass: Built,
    ghost: [Built; 2],
    draws_buffer: u32,
    draws_bytes: u32,
    bind_group: u32,
    light_group: u32,
    models: FrameGroups,
    shadow: Shadow,
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
    cases: [Option<Case>; SURFACES],
    order: Vec<u32>,
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
        let ao = ScreenAo::new(g, &mut pipelines, device.align, frame_buffer);
        let atmosphere = Atmosphere::new(g, &mut pipelines, frame_buffer, sky_buffer);
        let air = atmosphere.entries();
        let stars = StarPass::new(g, &mut pipelines, frame_buffer);
        let clouds = CloudPass::new(g, &mut pipelines, frame_buffer, air);
        let mips = Mips::new(g, &mut pipelines, format::RGBA8_SRGB);
        let layers = Layers::new(g);
        let chunk = pipelines.build(g, PIPELINE_CHUNK, format::CANVAS);
        let sky_built = pipelines.build(g, PIPELINE_SKY, format::CANVAS);
        let sky_group = object();
        let moon = ImageMap::new(g, MOON_TEX_SZ, MOON_TEX_SZ);
        let mut sky_entries = [0u32; 20 + AIR_WORDS];
        sky_entries[..5].copy_from_slice(&buffer_entry(0, frame_buffer));
        sky_entries[5..10].copy_from_slice(&buffer_entry(1, sky_buffer));
        sky_entries[10..15].copy_from_slice(&[2, resource::VIEW, moon.view, 0, 0]);
        sky_entries[15..20].copy_from_slice(&[3, resource::SAMPLER, moon.sampler, 0, 0]);
        sky_entries[20..].copy_from_slice(&air);
        g.bind_group(sky_group, sky_built.layouts[0], &sky_entries);
        let draws_buffer = object();
        g.buffer(draws_buffer, DRAWS_INITIAL * 4, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        let models = FrameGroups::new(
            g,
            frame_layout(g, &mut pipelines),
            frame_buffer,
            shadow.entries(1),
            Some((shadow.light_frame, shadow.blank_entries(1))),
            ao.blank_entry(7),
            air,
        );
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
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned: SkinnedPass::new(g, &mut pipelines, device.align),
            puck: PuckPass::new(g, &mut pipelines, frame_buffer, air),
            badges: BadgePass::new(g, &mut pipelines, frame_buffer),
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
            gpu_ms: 0.0,
            gpu_spans: [0.0; GPU_SPANS.len()],
        };
        r.chunk_groups(g);
        r
    }

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

    fn chunk_groups(&mut self, g: &Gpu) {
        release(self.bind_group);
        release(self.light_group);
        self.bind_group = self.chunk_group(g, self.frame_buffer, self.shadow.entries(1));
        self.light_group = self.chunk_group(g, self.shadow.light_frame, self.shadow.blank_entries(1));
    }

    /// The occlusion map is a new object, so every lit pass's group 0 is remade.
    fn rebind(&mut self, g: &Gpu) {
        self.chunk_groups(g);
        let light = Some((self.shadow.light_frame, self.shadow.blank_entries(1)));
        self.models.rebind(g, self.frame_buffer, self.shadow.entries(1), light, self.ao.entry(7));
    }

    fn lay_out(&mut self, g: &Gpu, width: u32, height: u32, env: &[f32]) {
        let s = stage_lane();
        let (mut a, mut m, mut proj) = (mat4(), mat4(), mat4());
        a.copy_from_slice(&s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16]);
        mat4_mul(&mut proj, &GL_TO_WGPU, &a);
        // The badges measure their size in view space.
        let b = &mut self.badges;
        put_mat(&mut b.view, 0, s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].try_into().or_bug(Errno::ENOTRECOVERABLE));
        put_mat(&mut b.proj, 0, &proj);
        b.px_k = (s[STAGE_AT::VIEWPORT + 1] / (2.0 * libm::tan(s[STAGE_AT::FOV] * core::f64::consts::PI / 360.0))) as f32;
        b.near = s[STAGE_AT::NEAR] as f32;
        b.far = s[STAGE_AT::FAR] as f32;
        b.cell = s[STAGE_AT::LOD_CELL] as f32;

        let u = &mut self.frame;
        a.copy_from_slice(&s[STAGE_AT::EYE_VIEW_PROJ..STAGE_AT::EYE_VIEW_PROJ + 16]);
        mat4_mul(&mut m, &GL_TO_WGPU, &a);
        put_mat(u, VIEW_PROJ, &m);
        // Everything is drawn in the eye's frame; positions are taken against the eye
        // in f64 before they are narrowed.
        let eye_local = [s[STAGE_AT::EYE_LOCAL], s[STAGE_AT::EYE_LOCAL + 1], s[STAGE_AT::EYE_LOCAL + 2]];
        let eye = [0.0; 3];
        let origin = ivec::gather(ivec::from_real(s[STAGE_AT::ORIGIN]), ivec::from_real(s[STAGE_AT::ORIGIN + 1]), 0);
        // The globe places by offset from its anchor and height alone, so placing in
        // the eye's frame matches the root frame less the eye.
        let center: [f64; 3] = core::array::from_fn(|k| s[STAGE_AT::GLOBE_ANCHOR + k] - eye_local[k]);
        let sphere = Globe::at(env[ENV_AIR + 2] as f64);
        let planet_center = [center[0], center[1], center[2] - sphere.radius_u()];
        let up = norm3(&sub3(&eye, &planet_center));
        self.clouds.lay_out(g, &m, &eye, env);
        self.atmosphere.set_clouds(&clouds::field(origin, [eye_local[0], eye_local[1]], env));
        self.stars.lay_out(g, &m, env, width);
        put_ray(&mut self.sky, 0, &m, &eye);
        self.sky[SKY_POLE..SKY_POLE + 3].copy_from_slice(&env[ENV_STARS + 8..ENV_STARS + 11]);
        self.sky[SKY_MAPPED] = if self.moon.loaded { 1.0 } else { 0.0 };
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
            u[UP + i] = up[i] as f32;
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
        u[AO_STRENGTH] = s[STAGE_AT::AO_STRENGTH] as f32;
        self.screen_ao = s[STAGE_AT::SCREEN_AO] != 0.0;
        u[SCREEN_AO] = if self.screen_ao { 1.0 } else { 0.0 };
        if self.screen_ao {
            self.ao.lay_out(g, &proj, width, height);
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
        put_globe(u, &globe);
        self.shadows = s[STAGE_AT::SHADOWS] != 0.0;
        u[SHADOW_STRENGTH] = if self.shadows { 1.0 } else { 0.0 };
        u[SHADOW_TEXEL] = self.shadow.texel as f32;
        if self.shadows {
            self.shadow.aim(g, &env[0..3], center, eye_local.map(|v| -v), globe);
            put_mat(u, LIGHT_VIEW_PROJ, &self.shadow.light_view_proj);
        }

        self.atmosphere.lay_out(g, env, &up);
    }

    /// A box is one instance shared by 36 vertices; a draw's first box is its first
    /// instance. Both vertex slots bind the same buffer so one shader serves both
    /// formats; the record index rides on the first vertex.
    fn draw_meshes(&self, g: &Gpu, pass: u32, draws: &Draws, which: u32, own: Option<u32>) {
        let b = blocks();
        let mut bound = 0;
        for d in draws.of(which) {
            let (handle, box_cnt, first_box, uniform) = (d[0], d[1], d[2], d[3]);
            let buffer = if matches!(which, PIPELINE_CHUNK | PIPELINE_CHUNK_COARSE) {
                arena_buffer(handle as usize)
            } else {
                b.get(handle).map(|(buffer, ..)| buffer)
            };
            // A missing buffer is a fault in the list. Skipping it silently once drew no
            // ground at all with every gauge reading normal.
            let Some(buffer) = buffer else { xpute_core::bug!(ENOTRECOVERABLE, handle, which) };
            if let (Some(p), 0) = (own, bound) {
                g.set_pipeline(pass, p);
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

    /// The impostors are the lit pass's alone. `bound` comes back as the group 0 the pass has.
    #[allow(clippy::too_many_arguments)]
    fn draw_opaque(&mut self, g: &Gpu, pass: u32, draws: &Draws, b: &mut Blocks, stage: Stage, chunk: [u32; 2], group: u32, models: u32) -> u32 {
        g.set_pipeline(pass, chunk[0]);
        g.set_bind_group(pass, 0, group, &[]);
        self.draw_meshes(g, pass, draws, PIPELINE_CHUNK, None);
        g.set_pipeline(pass, chunk[1]);
        self.draw_meshes(g, pass, draws, PIPELINE_CHUNK_COARSE, None);
        let mut bound = group;
        self.decor.draw(g, pass, draws, b, stage, (models, &mut bound));
        self.skinned.draw(g, pass, draws.of(PIPELINE_SKINNED), b, stage, (models, &mut bound));
        bound
    }

    fn frame(&mut self, g: &Gpu, world: &mut Surface, input: FrameInput) -> FrameStat {
        self.decor.set_models(input.decor);
        self.impostor.set_models(input.decor);
        for &(model, image) in input.skins {
            self.skinned.set_skin(g, model, image);
        }
        self.layers.step(g, &self.mips, input.sheet);
        self.moon.step(g, &self.mips, input.moon);
        self.atmosphere.earth_step(g, &self.mips, input.earth);
        let pages = input.earth_pages;
        self.atmosphere.set_earth_pages(g, pages.window, pages.layers, pages.upload);
        self.badges.sync_sheet(g, &self.mips, input.atlas, input.text);
        let (records, uniforms) = take_lists(Some(g), input.lists);

        targets(g, world);
        self.lay_out(g, world.width, world.height, input.environment);
        if self.screen_ao && self.ao.ensure(g, world.width, world.height) {
            self.rebind(g);
        }
        self.clouds.ensure(g, world.width, world.height);
        g.write_buffer(self.frame_buffer, 0, bytes(&self.frame));
        g.write_buffer(self.sky_buffer, 0, bytes(&self.sky));
        let uniform_bytes = word_bytes(uniforms.len());
        if uniform_bytes > self.draws_bytes {
            release(self.draws_buffer);
            self.draws_bytes = uniform_bytes * 2;
            self.draws_buffer = object();
            g.buffer(self.draws_buffer, self.draws_bytes, buffer_usage::STORAGE | buffer_usage::COPY_DST);
            self.chunk_groups(g);
        }
        if !uniforms.is_empty() {
            g.write_buffer(self.draws_buffer, 0, bytes(uniforms));
        }
        let mut order = core::mem::take(&mut self.order);
        let draws = Draws::sort(records.as_chunks::<DRAW_WORDS>().0, &mut order);
        let b = blocks();
        self.decor.prepare(g, &draws, uniforms, b);
        self.impostor.prepare(g, draws.of(PIPELINE_IMPOSTOR), uniforms, b);
        self.skinned.prepare(g, draws.of(PIPELINE_SKINNED), uniforms, b);
        self.puck.prepare_zone(g, &draws, b);
        self.badges.prepare(g, world.depth_view, &draws, b);
        self.debug.prepare(g, &draws);
        self.prepare_cases(g, &draws, uniforms, input.cases);
        let q = self.query_set;
        // A query no pass wrote holds an earlier frame's value, so which were written is passed along.
        let mut stamps: [Stamp; GPU_SPANS.len()] = core::array::from_fn(|span| Stamp::of(q, span));
        // What outlives the frame is drawn and submitted ahead of it: from here on
        // the frame only describes itself.
        let ahead = object();
        g.encoder(ahead);
        self.impostor.bake(g, ahead, &mut self.decor, b);
        self.atmosphere.draw(g, ahead, &mut stamps[SPAN_AIR]);
        g.submit(ahead);
        release(ahead);

        // The frame proper changes nothing that outlives it, so one too large
        // for the stream is dropped whole and the screen keeps the last.
        frame_begin();
        let encoder = object();
        g.encoder(encoder);
        if self.shadows {
            let pass = self.shadow.begin(g, encoder, &mut stamps[SPAN_SHADOW]);
            let chunk = [self.chunk_depth.pipeline, self.chunk_coarse_depth.pipeline];
            self.draw_opaque(g, pass, &draws, b, Stage::Depth, chunk, self.light_group, self.models.light);
            g.end(pass);
            release(pass);
        }
        self.clouds.march(g, encoder);
        if self.screen_ao {
            let pass = self.ao.begin_prepass(g, encoder, &mut stamps[SPAN_PREPASS]);
            let chunk = [self.chunk_prepass.pipeline, self.chunk_coarse_prepass.pipeline];
            self.draw_opaque(g, pass, &draws, b, Stage::Prepass, chunk, self.bind_group, self.models.frame);
            g.end(pass);
            release(pass);
            self.ao.run(g, encoder, &mut stamps[SPAN_AO]);
        }
        let canvas_view = object();
        g.current(world.handle, canvas_view);
        let pass = object();
        g.pass(pass, encoder, &pass_desc(canvas_view, load::CLEAR, world.depth_view, load::CLEAR, 0.0, stamps[SPAN_WORLD].pass()));
        let chunk = [self.chunk.pipeline, self.chunk_coarse.pipeline];
        let mut bound = self.draw_opaque(g, pass, &draws, b, Stage::Lit, chunk, self.bind_group, self.models.frame);
        // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
        self.impostor.draw(g, pass, draws.of(PIPELINE_IMPOSTOR), b, (self.models.frame, &mut bound));
        // At the far plane after the opaque world, so only empty pixels run the sky shader.
        g.set_pipeline(pass, self.sky_built.pipeline);
        g.set_bind_group(pass, 0, self.sky_group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.stars.draw(g, pass);
        self.clouds.draw(g, pass);
        self.puck.draw_beacon(g, pass, &draws, uniforms);
        self.puck.draw_zone(g, pass, &draws, uniforms, b);
        self.draw_meshes(g, pass, &draws, PIPELINE_GHOST_BOX, Some(self.ghost[0].pipeline));
        self.draw_meshes(g, pass, &draws, PIPELINE_GHOST_BOX_COPLANAR, Some(self.ghost[1].pipeline));
        let mut bound = 0;
        self.decor.draw_ghosts(g, pass, &draws, b, (self.models.frame, &mut bound));
        self.debug.draw(g, pass, &draws, uniforms, b);
        g.end(pass);
        release(pass);
        self.badges.draw(g, encoder, canvas_view, world.depth_view, &draws, uniforms, b, &mut stamps[SPAN_BADGES]);
        self.draw_cases(g, encoder, &draws, uniforms, input.cases, &mut stamps[SPAN_CASES]);
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
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
        }
        let stat = self.stat(draws.records, world, skipped);
        self.order = order;
        stat
    }

    /// The cases drawn this frame: a surface still a case, and its actor's draw.
    fn cases<'a, 'd>(draws: &'a Draws<'d>, cases: &'a [(u32, u32, u32)]) -> impl Iterator<Item = (u32, u32, u32, &'d [u32; DRAW_WORDS])> + 'a {
        cases.iter().filter_map(move |&(id, model, image)| {
            surface(id).filter(|s| s.role == surface_role::CASE)?;
            let own = draws.of(PIPELINE_SKINNED).find(|d| d[2] == model)?;
            Some((id, model, image, own))
        })
    }

    /// Each case's targets, passes and skin, made here so drawing makes nothing
    /// that outlives its frame.
    fn prepare_cases(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], cases: &[(u32, u32, u32)]) {
        for (id, model, image, own) in Self::cases(draws, cases) {
            let Some(s) = surface(id) else { continue };
            targets(g, s);
            let slot = handle_slot(id) as usize;
            if self.cases[slot].is_none() {
                self.cases[slot] = Some(Case::new(g, &mut self.pipelines, &self.shadow, &self.ao, self.atmosphere.entries(), self.align));
            }
            let case = self.cases[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            case.skinned.set_skin(g, model, image);
            case.skinned.prepare(g, core::iter::once(own), uniforms, blocks());
        }
    }

    fn draw_cases(&mut self, g: &Gpu, encoder: u32, draws: &Draws, uniforms: &[f32], cases: &[(u32, u32, u32)], stamp: &mut Stamp) {
        for (id, _, _, own) in Self::cases(draws, cases) {
            let Some(s) = surface(id) else { continue };
            let Some(case) = self.cases[handle_slot(id) as usize].as_mut() else { continue };
            let world_frame = self.frame;
            let u = uniform_of(uniforms, own);
            let actor = [u[0] as f64, u[1] as f64, u[2] as f64];
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
            // No fog: near and far pushed out of reach.
            u[ENV + ENV_FOG_NEAR] = 1e9;
            u[ENV + ENV_FOG_FAR] = 2e9;
            u[VIEWPORT] = s.width as f32;
            u[VIEWPORT + 1] = s.height as f32;
            u[SHADOW_STRENGTH] = 0.0;
            u[VIGNETTE] = 0.0;
            u[SCREEN_AO] = 0.0;
            // No planet: the case is a room of its own.
            u[PLANET + 2] = 0.0;
            g.write_buffer(case.frame_buffer, 0, bytes(&case.frame));
            let view_id = object();
            g.current(s.handle, view_id);
            let pass = object();
            g.pass(pass, encoder, &pass_desc(view_id, load::CLEAR, s.depth_view, load::CLEAR, 0.0, stamp.pass()));
            case.skinned.draw(g, pass, core::iter::once(own), blocks(), Stage::Lit, (case.frames.frame, &mut 0));
            g.end(pass);
            release(pass);
            release(view_id);
        }
    }

    /// What the scheduler asked for: a draw passed over because its block, skin or
    /// sprite is not on the device yet is still counted.
    fn stat(&self, draws: &[[u32; DRAW_WORDS]], world: &Surface, skipped: bool) -> FrameStat {
        let b = blocks();
        let mut triangles = 0u32;
        let mut census = [[0.0; 2]; PIPELINES as usize];
        for d in draws {
            let tris = match d[4] {
                PIPELINE_CHUNK | PIPELINE_CHUNK_COARSE | PIPELINE_GHOST_BOX | PIPELINE_GHOST_BOX_COPLANAR => d[1] * 12,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
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
        FrameStat {
            gpu_ms: self.gpu_ms,
            draw_calls: draws.len() as f64,
            triangles: triangles as f64,
            buffers: b.count as f64,
            buffer_bytes: b.bytes as f64,
            depth_mb: round(px * 4.0 / MB),
            shadow_mb: round(self.shadow.bytes as f64 / MB),
            layers_mb: round(self.layers.bytes as f64 / MB),
            px,
            census,
            gpu_spans: self.gpu_spans,
            arena_mb: round(arena_bytes() as f64 / MB),
            skipped: f64::from(u8::from(skipped)),
        }
    }
}

/// A new size is applied once it has held SETTLE_TURNS turns; until then the
/// page scales the old buffer.
fn targets(g: &Gpu, s: &mut Surface) {
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

/// Without a device or the world's canvas, the lists are recorded for the frame that has both.
pub fn render_frame(input: FrameInput) -> FrameOutput {
    REDRAW.set(false);
    let world = surfaces().find(|s| s.role == surface_role::WORLD);
    let (Some(g), Some(device), Some(world)) = (gpu(), *DEVICE.get(|| None), world) else {
        take_lists(RENDERER.get(|| None).as_ref().and(gpu()), input.lists);
        return FrameOutput { redraw: false, stat: None };
    };
    // The GPU holds as many frames as it may: skip rather than wait.
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

/// Blocks listed with no device wait in the table for the next one.
fn take_lists<'a>(g: Option<&Gpu>, lists: Lists<'a>) -> (&'a [u32], &'a [f32]) {
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
    (lists.draws, lists.uniforms)
}
