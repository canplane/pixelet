// pxl-render/gpu/renderer.rs

//! Executes the scheduler's draw lists on the device, in the kernel's own turn,
//! in the device's depth (`mat4::perspective`).

use crate::gpu::span_spec::{FrameKind, GpuSpanName, GPU_SPAN_NAMES};
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_kit::math::scalar::round;

use crate::api_spec::surface_role;
use crate::frame::draw::{
    census_slot, uniform_of, DrawCensus, Lists, DRAW_PIPELINES, DRAW_WORDS, PIPELINE_BADGE, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED, REWRITE_WORDS, UPLOAD_WORDS,
};
use crate::frame::shader::record::{compute, dynamic, same, within};
use crate::frame::shader::record::{
    FRAME_AT, PIPELINES, PIPELINE_BLIT, PIPELINE_DEBUG_FACE, PIPELINE_SCREEN_RAYS, PIPELINE_SKY, SHADER_BOUND, SHADER_CAST, SHADER_HEIGHTS, SHADER_JOINED, SHADER_START, SHADER_WAYS, SHADER_WAYS_FREE,
    SHADER_WAYS_FREED, SHADER_WAYS_TREE,
};
use crate::frame::shader::words::{DRAW_FLOATS, FRAME_FLOATS, MOON_TEXELS, SKY_FLOATS};
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::ao::ScreenAo;
use crate::gpu::api::{
    buffer_entry, buffer_range_entry, buffer_usage, bytes, device, device_lost, frame_begin, frame_commit, gpu, pass_desc, release, room, sampler_entry, texture_usage, view_entry, word_bytes,
    write_buffer_bytes, Device, Entry, Format, Gpu, Load, Stamp, DONE,
};
use crate::gpu::atmosphere::{Atmosphere, AIR_ENTRIES};
use crate::gpu::clouds::CloudPass;
use crate::gpu::grown::{DecorFrame, Drawn, Grown, Grows};
use crate::gpu::image_map::ImageMap;
use crate::gpu::layers::{Layers, Mips};
use crate::gpu::pass::badges::BadgePass;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::impostor::{Extent, ImpostorPass};
use crate::gpu::pass::model::{frame_layout, FrameGroups, Stage};
use crate::gpu::pass::puck::PuckPass;
use crate::gpu::pass::skinned::SkinnedPass;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::sheet::shelf::Sheet;
use crate::gpu::stars::StarPass;
use crate::gpu::surface::{surface, surfaces, surfaces_lost, Surface, SETTLE_TURNS, SURFACES};
use crate::gpu::text::TextOf;
use crate::look::environment::{ENV_AIR, ENV_MOON_LIGHT, ENV_STARS, ENV_SUN_LIGHT, LIGHT_COLOR};
use crate::mesh::paint::ground::look_words;
use pxl_portable::eye;
use pxl_portable::ray;
use pxl_portable::scatter::GROUPS;
use pxl_portable::ways;
use pxl_portable::window::{self, CellsBack, FreshSquare, BACK_WORDS, COLUMNS_AT, ELE_AT, HANDED, HANDED_WORDS, LEVELS, LINES_AT, LOOKS_AT, SQUARE, SQUARES, SQUARE_CELLS, SQUARE_WORDS};
use pxl_spec::space::coord::globe::MERCATOR_RADIUS_U;
use pxl_spec::space::coord::world;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub(crate) const VIEW_PROJ: usize = FRAME_AT[0];
const CAMERA_POS: usize = FRAME_AT[1];
const EDGE_STRENGTH: usize = FRAME_AT[2];
const SUN_DIR: usize = FRAME_AT[3];
const LOW_SPEC: usize = FRAME_AT[4];
const SUN_COLOR: usize = FRAME_AT[5];
const EDGE_WIDTH: usize = FRAME_AT[6];
const OTHER_DIR: usize = FRAME_AT[7];
const RIM_INTENSITY: usize = FRAME_AT[8];
const OTHER_COLOR: usize = FRAME_AT[9];
const VIGNETTE: usize = FRAME_AT[10];
const VIEWPORT: usize = FRAME_AT[11];
const SCREEN_AO: usize = FRAME_AT[12];
const MAP: usize = FRAME_AT[13];
const CELL: usize = FRAME_AT[14];

/// The ray, the celestial pole, and whether the Moon's face has arrived.
pub(crate) const SKY_POLE: usize = 12;
pub(crate) const SKY_MAPPED: usize = 15;
const BOX_VERTICES: u32 = 36;
const DRAWS_INITIAL: u32 = 256 * DRAW_FLOATS as u32;
const MB: f64 = 1_048_576.0;

const _: () = assert!(GPU_SPAN_NAMES.len() <= 32, "a span a bit of the mask the host is told");
const SPAN_AIR: usize = GpuSpanName::air as usize;
const SPAN_PREPASS: usize = GpuSpanName::prepass as usize;
const SPAN_AO: usize = GpuSpanName::ao as usize;
const SPAN_WORLD: usize = GpuSpanName::world as usize;
const SPAN_OVERLAY: usize = GpuSpanName::overlay as usize;
const SPAN_BADGES: usize = GpuSpanName::badges as usize;
const SPAN_DEBUG: usize = GpuSpanName::debug as usize;
const SPAN_CASES: usize = GpuSpanName::cases as usize;
const SPAN_FILL: usize = GpuSpanName::fill as usize;
const SPAN_START: usize = GpuSpanName::start as usize;
const SPAN_CAST: usize = GpuSpanName::cast as usize;
const SPAN_GROW: usize = GpuSpanName::grow as usize;

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
    /// Whether its GPU spans are timed and read back (GPU_SPAN_NAMES): only
    /// for a reader of them, as their read's landing wakes a turn.
    pub timed: bool,
    /// Whether its draws are counted (`FrameStat::census`): the profiler's
    /// alone, so a frame it does not record counts none.
    pub counted: bool,
}

/// The buffers squares' cells come back through: what a frame hands, a
/// mapping still out a frame more (pxl-portable `window::HANDED`).
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

/// Hands each waiting tile's layer and texels to the writer while it takes them.
pub type Upload = dyn FnMut(&mut dyn FnMut(u32, &[u8]) -> bool);

/// The ground's windows as the GPU holds them (pxl-portable `window`,
/// `SQUARES` of them): the squares to hand it, and where their cells come
/// back.
pub struct GroundSquares<'a> {
    /// Hands each square whose cells the GPU is to draw to the writer.
    pub upload: &'a mut TerrainUpload,
    /// Told each square whose cells came back, and the fill they are of
    /// (`CellsBack::ticket`).
    pub landed: &'a mut CellsLanded,
    /// Where the eye's windows folded over their squares come back to in
    /// the module's memory (`joined_cs`; pxl-portable
    /// `window::EYE_JOINED_WORDS`).
    pub joined: u32,
    /// Where each read's records come back to in the module's memory, read
    /// `k`'s at `records + k · ways::tree::RECORDS_BYTES`: the blocks its
    /// square's ways' tree holds (gpu-query-shader `ways_tree_cs`).
    pub records: u32,
}

/// A square whose cells came back, the fill they are of
/// (`CellsBack::ticket`), and the read they came through, whose records'
/// room holds its ways' blocks (`GroundSquares::records`).
pub type CellsLanded = dyn FnMut(u32, u32, u32);

/// The GPU's cast this frame (frame/shader/cast.gen.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-portable `eye`; pxl-kernel view/lens.rs `gpu_cast`,
    /// scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// Per window of the eye's, from the top's, a row a slot: the square it
    /// holds, X and Y, and whether it is present, and a spare.
    pub window: &'a [i32],
    /// The service area's index (pxl-portable `area::Area`), its keys four a
    /// row (pxl-kernel scene/cast.rs `shards`).
    pub shards: &'a [[i32; 4]],
    pub overwrites: Overwrites<'a>,
    /// The decor the GPU grows and draws over the same world, none where
    /// decor is off (gpu/grown.rs).
    pub decor: Option<DecorFrame<'a>>,
}

/// The overwrites' tree (pxl-kernel layer/object/store.rs) as it lives: its
/// heap and data lanes, unwritten until the page replays the turn, the room
/// either has before it grows, and the revision they are at.
pub struct Overwrites<'a> {
    pub heap: &'a [u32],
    pub data: &'a [u32],
    pub room: usize,
    pub revision: u32,
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
    /// read's, and else an earlier frame's, not this one's.
    pub gpu_read: bool,
    /// The block table's blocks and their bytes.
    pub blocks: f64,
    pub block_bytes: f64,
    pub depth_mb: f64,
    pub layers_mb: f64,
    pub world_px: f64,
    /// Every draw's, counted only while `counted`: its sums are the frame's
    /// draws and triangles.
    pub census: DrawCensus,
    pub gpu_spans: [f64; GPU_SPAN_NAMES.len()],
}

/// A frame's GPU time: its spans' sum, the passes timed. The span from the
/// first to the last would count what lies between submits.
pub fn gpu_ms(spans: &[f64]) -> f64 {
    spans.iter().sum()
}

pub fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_mat(out: &mut [f32], at: usize, m: &Mat4) {
    for (i, v) in m.iter().enumerate() {
        out[at + i] = *v as f32;
    }
}

/// The ray's words `put_ray` writes: its three columns, a vec4 each.
pub(crate) const RAY_FLOATS: usize = 12;

/// The ray under NDC (x, y) is `x·col0 + y·col1 + col2`, taken in f64. Un-
/// projecting two points in the shader kept a direction to only ~1e-3 rad,
/// which the secant carried out to kilometers at the horizon: the clouds shook.
pub(crate) fn put_ray(out: &mut [f32], at: usize, view_proj: &Mat4, eye: &V3) {
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

pub fn frame_group(g: &Gpu, layout: u32, frame: u32, ao: Entry, air: [Entry; AIR_ENTRIES]) -> u32 {
    let mut e = vec![buffer_entry(0, frame), ao];
    e.extend(air);
    g.bind_group(layout, &e)
}

/// An image file as its asker hands it: its name, the asker's own and the
/// same for one file wherever it is handed, and its bytes, handed for as
/// long as the image is wanted, so a device gone takes it again from them
/// (`Gpu::image`).
pub type Image = (u32, &'static [u8]);

/// Each skin image as one texture's view, made the first time a skin binds it:
/// an image's texture is taken once (`Gpu::image`), so
/// a skin made again, or by another pass, binds what is here. As many as the
/// skins the asset table names.
static SKIN_VIEWS: Global<Vec<(u32, u32)>> = Global::new();

fn skin_view(g: &Gpu, (image, file): Image) -> Option<u32> {
    let views = SKIN_VIEWS.get(Vec::new);
    if let Some(&(_, view)) = views.iter().find(|&&(i, _)| i == image) {
        return Some(view);
    }
    let texture = g.image(image, file)?;
    let view = g.view(texture, false, 0, 1, 0, 1);
    views.push((image, view));
    Some(view)
}

/// A skin's bind group; none while its image lands.
pub fn skin_group(g: &Gpu, layout: u32, sampler: u32, image: Image) -> Option<u32> {
    Some(g.bind_group(layout, &[view_entry(0, skin_view(g, image)?), sampler_entry(1, sampler)]))
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

/// Its block goes with it: the renderer is dropped whole when the GPU is lost.
impl Drop for Scratch {
    fn drop(&mut self) {
        self.free();
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
    count: u32,
    bytes: u64,
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

/// The table's lanes, then the draw list's (frame/draw.rs `lanes_bytes`),
/// room for the most blocks live, half the slots.
pub const fn blocks_bytes(slots: usize) -> u32 {
    (BLOCK_LANES * slots * 4) as u32 + crate::frame::draw::lanes_bytes(slots / 2)
}

/// # Safety
/// `at` must address `blocks_bytes(slots)`, 4-aligned and zeroed, the
/// renderer's alone for as long as the module runs.
pub unsafe fn set_blocks(at: u32, slots: usize) {
    xpute_kit::ensure!(slots.is_power_of_two(), EINVAL, slots);
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
    // SAFETY: the range's rest past the table, the lists' alone.
    unsafe { crate::frame::draw::set_lanes(at + (BLOCK_LANES * slots * 4) as u32, slots / 2) };
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
        if !room(write_buffer_bytes(memory(self.at[i], self.len[i] & !3))) {
            return false;
        }
        let buffer = g.buffer(buffer_size(self.len[i]), buffer_usage::VERTEX | buffer_usage::INDEX | buffer_usage::STORAGE | buffer_usage::COPY_DST);
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
        xpute_kit::ensure!((self.count as usize) < self.handle.len() / 2, ENOTRECOVERABLE, self.handle.len() / 2);
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
        if room(write_buffer_bytes(memory(at + from, bytes))) {
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

    /// For a draw binding it as a vertex buffer: the ghost's boxes are read through it (box.wgsl).
    pub fn group(&mut self, g: &Gpu, handle: u32, layout: u32) -> Option<u32> {
        let i = self.find(handle).filter(|&i| self.buffer[i] != 0)?;
        if self.group[i] == 0 {
            let id = g.bind_group(layout, &[buffer_entry(0, self.buffer[i])]);
            self.group[i] = id;
        }
        Some(self.group[i])
    }
}

static BLOCKS: Global<Blocks> = Global::new();

fn blocks() -> &'static mut Blocks {
    BLOCKS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}

static RENDERER: Global<Option<Renderer>> = Global::new();

/// Each light's step, the sun's first: the height it moves before the sky as
/// drawn moves a display level (gpu/atmosphere.rs `steps`); none before any
/// view's slope has landed.
pub fn sky_steps() -> [Option<f32>; 2] {
    RENDERER.get(|| None).as_ref().map_or([None; 2], |r| r.atmosphere.steps())
}

static REDRAW: Global<bool> = Global::new();

/// WebGPU's default minUniformBufferOffsetAlignment: an adapter's limit is a
/// power of two no larger, so a larger one is no device's.
pub const UNIFORM_ALIGN_MAX: u32 = 256;

pub fn gpu_lost() {
    RENDERER.set(None);
    SKIN_VIEWS.set(Vec::new());
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
/// of the world hides one.
///
/// Each set takes its own translation, so a buffer and a group a set, grown
/// to the most sets a frame has drawn.
struct DebugPass {
    built: Built,
    faces: Built,
    frame_group: u32,
    sets: Vec<(u32, u32)>,
}

impl DebugPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, g.canvas());
        let faces = pipelines.build(g, PIPELINE_DEBUG_FACE, g.canvas());
        let frame_group = g.bind_group(built.layouts[0], &[buffer_entry(0, frame_buffer)]);
        DebugPass {
            built,
            faces,
            frame_group,
            sets: Vec::new(),
        }
    }

    /// A set for each set of lines, made here so drawing makes nothing that
    /// outlives its frame.
    fn prepare(&mut self, g: &Gpu, draws: &Draws) {
        while self.sets.len() < draws.of(PIPELINE_DEBUG).len() {
            let buffer = g.buffer(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            let group = g.bind_group(self.built.layouts[1], &[buffer_entry(0, buffer)]);
            self.sets.push((buffer, group));
        }
    }

    /// `depth` is the world's, which nothing reads after this.
    #[allow(clippy::too_many_arguments)]
    /// Each set's uniform written where the lists it is made from `moved`.
    fn draw(&self, g: &Gpu, encoder: u32, canvas: u32, depth: u32, draws: &Draws, uniforms: &[f32], moved: bool, blocks: &Blocks, stamp: &mut Stamp) {
        if draws.of(PIPELINE_DEBUG).len() == 0 {
            return;
        }
        // Each set held: its own uniform and bind group, its line vertices and its face vertices.
        let sets = || {
            draws.of(PIPELINE_DEBUG).enumerate().filter_map(|(k, d)| {
                let (block, _, _) = blocks.get(d[0])?;
                (d[1] > 0).then_some((k, d, block, d[1], d[2]))
            })
        };
        for (k, d, ..) in sets().filter(|_| moved) {
            let u = uniform_of(uniforms, d);
            g.write_buffer(self.sets[k].0, 0, bytes(&[u[0], u[1], u[2], 0.0]));
        }
        // With no faces the lines' pass clears the depth itself.
        let faced = sets().any(|s| s.4 > 0);
        if faced {
            let pass = g.pass(encoder, &pass_desc(0, Load::Clear, depth, Load::Clear, 0.0, stamp.pass()));
            g.set_pipeline(pass, self.faces.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, block, lines, faces) in sets().filter(|s| s.4 > 0) {
                g.set_bind_group(pass, 1, self.sets[k].1, &[]);
                g.set_vertex_buffer(pass, 0, block, lines * 28, faces * 12);
                g.draw(pass, faces, 1, 0, 0);
            }
            g.end(pass);
        }
        let depth_load = if faced { Load::Load } else { Load::Clear };
        let pass = g.pass(encoder, &pass_desc(canvas, Load::Load, depth, depth_load, 0.0, stamp.pass()));
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        for (k, _, block, lines, _) in sets() {
            g.set_bind_group(pass, 1, self.sets[k].1, &[]);
            g.set_vertex_buffer(pass, 0, block, 0, lines * 28);
            // A segment an instance: its two vertices, read as one.
            g.draw(pass, 6, lines / 2, 0, 0);
        }
        g.end(pass);
    }
}

/// The eye's rays, each drawn at its own pixel where it met a face, painted
/// by paint.wgsl's `paint` (pxl-portable ray.rs), from the world surface's
/// `rays` the GPU's cast leaves (`Cast`).
struct RaysPass {
    built: Built,
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
    /// it is handed (pxl-portable `window::SQUARE_WORDS`).
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
    /// Each read's records (`ways::tree::RECORDS_BYTES`), mapped beside
    /// its square's words, and its mapping's word: a read lands with both.
    record_reads: [u32; READS],
    record_mapping: Box<[u32; READS]>,
    /// The ways' pool, which the cast reads, and the handed squares'
    /// scratch, whose records each read copies (`ways::tree::SCRATCH_WORDS`).
    pool: u32,
    scratch: u32,
    heights_built: Built,
    heights_group: u32,
    bound_built: Built,
    /// `bound_cs`'s group, its level at a dynamic offset (`SLOT_STRIDE`).
    bound_group: u32,
    /// `ways_cs`'s, and its group, its pass at a dynamic offset.
    ways_built: Built,
    ways_group: u32,
    /// A re-filled square's old ways' tree given back, its new one built,
    /// the blocks the build gave back pushed (pxl-portable `ways::tree`):
    /// each pass and its group, over the ways' pool, heap words then data
    /// words (`POOL_BYTES`), its free stack, the count then the ids, block 0
    /// never among them (`STACK_WORDS`), and each handed square's scratch
    /// past its build (`ways::tree::SCRATCH_WORDS`).
    free_built: Built,
    free_group: u32,
    tree_built: Built,
    tree_group: u32,
    freed_built: Built,
    freed_group: u32,
}

/// The ways' pool on the GPU, in bytes: a slot two words, eight a block.
const POOL_BYTES: u64 = ways::tree::POOL_BLOCKS as u64 * 8 * 2 * 4;
const _: () = assert!(
    crate::frame::shader::floor::fits(POOL_BYTES, true),
    "POOL_BYTES: the ways' pool binds past WebGPU's floor (frame/shader/floor.rs `fits`)"
);
/// Its free stack: the count, then every block's id but block 0's.
const STACK_WORDS: u32 = ways::tree::POOL_BLOCKS;
/// The stack as it starts: every block free but block 0, which names none.
fn stack_start() -> Vec<u32> {
    let mut w: Vec<u32> = (0..STACK_WORDS).collect();
    w[0] = STACK_WORDS - 1;
    w
}

/// What the GPU's cast reads; it writes the world surface's rays.
struct CastBuffers {
    built: Built,
    /// Where its rays start, asked before it (`start_cs`), over its group.
    start_built: Built,
    starts: u32,
    /// The eye's windows folded over their squares (`joined_cs`), over its
    /// group, the window buffer's rows past their slot table
    /// (`window::JOINED_AT`), written where the windows' slots moved
    /// (`refold`); then copied into `joined_read` and mapped back
    /// (`read_back`), once a mapping is free, while `joined_back`; the
    /// mapping's own word where the guest's allocator puts it, as a
    /// square's is.
    joined_built: Built,
    joined_read: u32,
    joined_mapping: Box<u32>,
    refold: bool,
    joined_back: bool,
    /// Its group 0, and the buffers it binds (`cast_group`): made again
    /// exactly when one of them is another.
    group: u32,
    bound: [u32; 9],
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

/// A stage's whole input as it last read it, byte for byte: what the stage
/// makes is one function of it, so a frame that reads the same stands on
/// what it made — the cast's rays and the decor grown over them (stage 1),
/// the world image (stage 2). Each lane is read with its length, so no two
/// sets read alike; what changed without words to read is marked so.
#[derive(Default)]
pub(crate) struct Read {
    bytes: Vec<u8>,
    at: usize,
    same: bool,
}

impl Read {
    pub(crate) fn begin(&mut self) {
        (self.at, self.same) = (0, true);
    }

    /// Whether `lane` reads otherwise than it did at this place last time.
    pub(crate) fn take(&mut self, lane: &[u8]) -> bool {
        let end = self.at + lane.len();
        let moved = self.bytes.get(self.at..end) != Some(lane);
        if moved {
            self.same = false;
            if self.bytes.len() < end {
                self.bytes.resize(end, 0);
            }
            self.bytes[self.at..end].copy_from_slice(lane);
        }
        self.at = end;
        moved
    }

    /// Whether `lane`, with its length, moved: what its writer needs to
    /// write again, and no more.
    pub(crate) fn read<T: Copy>(&mut self, lane: &[T]) -> bool {
        self.take(bytes(&[lane.len() as u32])) | self.take(bytes(lane))
    }

    /// What this frame changed of the stage's input that has no words, as
    /// an image come or a table drawn.
    pub(crate) fn mark(&mut self, changed: bool) {
        self.same &= !changed;
    }

    /// Whether all of it read as the last time.
    pub(crate) fn end(&mut self) -> bool {
        let same = self.same && self.at == self.bytes.len();
        self.bytes.truncate(self.at);
        same
    }

    /// The rays no longer hold what was read: the next frame casts.
    pub(crate) fn forget(&mut self) {
        self.bytes.clear();
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
/// by the cast at a hit (gpu-query-shader `World`'s `Looks`). Their own
/// buffer, as the squares' binding has no room for them.
const LOOKS_BYTES: u64 = SQUARES as u64 * SQUARE_CELLS as u64 * 4;
const _: () = assert!(
    crate::frame::shader::floor::fits(LOOKS_BYTES, true),
    "LOOKS_BYTES: the squares' looks bind past WebGPU's floor (frame/shader/floor.rs `fits`)"
);

/// `bound_cs`'s and `ways_cs`'s one group: its level or its pass at a
/// dynamic offset (`SLOT_STRIDE`), the fresh frames and the frames it writes
/// the levels or the runs of. The cast's, `start_cs`'s and `joined_cs`'s one
/// group is the cast's, which binds all theirs.
const BOUND_GROUP: [u32; compute::BOUND_GROUP.len()] = dynamic(compute::BOUND_GROUP, 0);
const _: () = assert!(same(compute::BOUND_GROUP, compute::WAYS_GROUP), "the levels' and the ways' passes share a group");
const _: () = assert!(compute::WAYS_WORKGROUP[0] == pxl_portable::fill::SCAN_THREADS, "a square's columns are summed by one workgroup");
const _: () = assert!(
    within(compute::START_GROUP, compute::CAST_GROUP) && within(compute::JOINED_GROUP, compute::CAST_GROUP),
    "the start and the fold bind the cast's group"
);

/// A number's slot in the fill's numbers, read at a dynamic offset: every
/// device's dynamic offset alignment.
const SLOT_STRIDE: u32 = UNIFORM_ALIGN_MAX;

impl RaysPass {
    fn new(g: &Gpu, pipelines: &mut Pipelines) -> RaysPass {
        let built = pipelines.build(g, PIPELINE_SCREEN_RAYS, g.canvas());
        let words = look_words();
        let looks = g.buffer(word_bytes(4 * words.len()), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(looks, 0, bytes(&words));
        RaysPass {
            built,
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
    fn hand(&mut self, g: &Gpu, pipelines: &mut Pipelines, upload: &mut TerrainUpload) {
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
            let entries = [buffer_range_entry(0, slots, 0, 16), buffer_entry(1, fresh), buffer_entry(2, frames_buffer)];
            let (bound_built, ways_built) = (pipelines.build_compute(g, SHADER_BOUND, &BOUND_GROUP), pipelines.build_compute(g, SHADER_WAYS, &BOUND_GROUP));
            let (bound_group, ways_group) = (g.bind_group(bound_built.layouts[0], &entries), g.bind_group(ways_built.layouts[0], &entries));
            let pool = buffer(POOL_BYTES as u32, buffer_usage::STORAGE);
            let stack = buffer(word_bytes(STACK_WORDS as usize), storage);
            g.write_buffer(stack, 0, bytes(&stack_start()));
            let scratch = buffer(word_bytes((HANDED * ways::tree::SCRATCH_WORDS) as usize), buffer_usage::STORAGE | buffer_usage::COPY_SRC);
            let free_built = pipelines.build_compute(g, SHADER_WAYS_FREE, &compute::WAYS_FREE_GROUP);
            let free_group = g.bind_group(
                free_built.layouts[0],
                &[buffer_entry(0, fresh), buffer_entry(1, frames_buffer), buffer_entry(2, pool), buffer_entry(3, stack)],
            );
            let tree_built = pipelines.build_compute(g, SHADER_WAYS_TREE, &compute::WAYS_TREE_GROUP);
            let tree_group = g.bind_group(
                tree_built.layouts[0],
                &[
                    buffer_entry(0, fresh),
                    buffer_entry(1, frames_buffer),
                    buffer_entry(2, pool),
                    buffer_entry(3, stack),
                    buffer_entry(4, scratch),
                ],
            );
            let freed_built = pipelines.build_compute(g, SHADER_WAYS_FREED, &compute::WAYS_FREED_GROUP);
            let freed_group = g.bind_group(freed_built.layouts[0], &[buffer_entry(0, scratch), buffer_entry(1, stack)]);
            GroundBuffers {
                ground: frames_buffer,
                looks,
                fresh,
                fresh_len: 0,
                asked: [CellsBack::default(); SQUARES as usize],
                handed: [0; HANDED as usize],
                reads: core::array::from_fn(|_| buffer(BACK_WORDS * 4, buffer_usage::MAP_READ | buffer_usage::COPY_DST)),
                held: [(FREE, 0); READS],
                mapping: Box::new([0; READS]),
                record_reads: core::array::from_fn(|_| buffer(ways::tree::RECORDS_BYTES, buffer_usage::MAP_READ | buffer_usage::COPY_DST)),
                record_mapping: Box::new([0; READS]),
                pool,
                scratch,
                heights_built,
                heights_group,
                bound_built,
                bound_group,
                ways_built,
                ways_group,
                free_built,
                free_group,
                tree_built,
                tree_group,
                freed_built,
                freed_group,
            }
        });
        // A square handed takes a read its frame: none past those free.
        let free = gb.held.iter().filter(|h| h.0 == FREE).count().min(HANDED as usize) as u32;
        let mut fresh = 0;
        upload(&mut |q| {
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

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels): the GPU's cast where an eye casts, its eye
    /// handed over first.
    fn prepare(&mut self, g: &Gpu, pipelines: &mut Pipelines, cast: Option<Cast>, meshes: &[u32; GROUPS as usize], rays: (u32, [u32; 2])) {
        // No cast, no world: no ray cast, none drawn and no decor grown.
        // What the cast reads stands for the next: the ground it binds is
        // made once.
        let (Some(cast), Some((ground, looks, pool))) = (cast, self.ground.as_ref().map(|gb| (gb.ground, gb.looks, gb.pool))) else {
            self.kind = FrameKind::plain;
            if let Some(c) = self.cast.as_mut() {
                c.grown.off();
                c.read.forget();
            }
            return;
        };
        if self.cast.is_none() {
            let built = pipelines.build_compute(g, SHADER_CAST, &compute::CAST_GROUP);
            let start_built = pipelines.build_compute(g, SHADER_START, &compute::CAST_GROUP);
            let joined_built = pipelines.build_compute(g, SHADER_JOINED, &compute::CAST_GROUP);
            let buffer = |size: u32, usage: u32| g.buffer(size, usage);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            let c = CastBuffers {
                built,
                start_built,
                starts: buffer(word_bytes(pxl_portable::cast::START_WORDS as usize), buffer_usage::STORAGE),
                joined_built,
                joined_read: buffer(word_bytes(window::EYE_JOINED_WORDS as usize), buffer_usage::MAP_READ | buffer_usage::COPY_DST),
                joined_mapping: Box::new(DONE),
                refold: false,
                joined_back: false,
                group: 0,
                bound: [0; 9],
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
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The cast reads exactly this, all of it here: a field the frame hands
        // the cast fails to destructure until it is read. The overwrites'
        // contents are their revision's. The decor it grows reads its own
        // (`Grown::prepare`), so a change to decor alone casts no ray again.
        let Cast {
            eye,
            window,
            shards,
            overwrites: Overwrites { heap: _, data: _, room, revision },
            decor: _,
        } = &cast;
        let r = &mut c.read;
        r.begin();
        // Each lane written again where it moved.
        let eye_moved = r.read(eye);
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
        if eye_moved {
            g.write_buffer(c.eye, 0, bytes(&cast.eye));
        }
        c.size = rays.1;
        let shards_grow = cast.shards.len() > c.shard_room;
        if shards_grow {
            if c.shards != 0 {
                release(c.shards);
            }
            c.shard_room = cast.shards.len();
            c.shards = g.buffer(word_bytes(4 * c.shard_room), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        }
        if shards_moved || shards_grow {
            g.write_buffer(c.shards, 0, bytes(cast.shards));
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
        if c.overwrite_revision != Some(o.revision) {
            g.upload_buffer(c.overwrites, 0, bytes(o.heap));
            g.upload_buffer(c.overwrites, word_bytes(o.room), bytes(o.data));
            c.overwrite_revision = Some(o.revision);
        }
        let bound = [c.eye, c.window, ground, rays.0, looks, c.shards, c.overwrites, c.starts, pool];
        if bound != c.bound {
            release(c.group);
            c.group = cast_group(g, &c.built, bound);
            c.bound = bound;
        }
        let fresh_squares = self.ground.as_ref().map_or(0, |gb| gb.fresh);
        let world = [c.eye, c.window, ground, c.shards, c.overwrites, fresh_squares];
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
        // before its build writes another (pxl-portable `ways::tree`).
        g.set_pipeline(pass, c.free_built.pipeline);
        g.set_bind_group(pass, 0, c.free_group, &[]);
        g.dispatch(pass, 1, 1, c.fresh_len);
        g.set_pipeline(pass, c.heights_built.pipeline);
        g.set_bind_group(pass, 0, c.heights_group, &[]);
        let w = compute::HEIGHTS_WORKGROUP;
        g.dispatch(pass, SQUARE.div_ceil(w[0]), SQUARE.div_ceil(w[1]), c.fresh_len);
        // A column a thread, over the tops just drawn: the columns' runs
        // counted, summed a square a workgroup (pxl-portable
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
    fn read_back(&mut self, g: &Gpu, landed: &mut CellsLanded, joined: u32, records: u32) {
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
        // fill asked again, whose own read brings it.
        for k in 0..READS {
            let (s, ticket) = c.held[k];
            if s == FREE || c.mapping[k] & DONE == 0 || c.record_mapping[k] & DONE == 0 {
                continue;
            }
            let back = &mut c.asked[s as usize];
            if back.ticket == ticket {
                landed(s, ticket, k as u32);
                back.ticket = 0;
            }
            c.held[k] = (FREE, 0);
        }
        // Each square the frame handed, its words and its build's records
        // copied in its frame, into a read `hand` kept free for it.
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
            g.copy_buffer(e, (c.ground, s * SQUARE_BYTES + COLUMNS_AT * 4), (c.reads[k], 0), BACK_WORDS * 4);
            g.copy_buffer(
                e,
                (c.scratch, word_bytes((h * ways::tree::SCRATCH_WORDS + ways::tree::RECORDS_AT) as usize)),
                (c.record_reads[k], 0),
                ways::tree::RECORDS_BYTES,
            );
            (c.held[k], taken[k]) = ((s, c.asked[s as usize].ticket), true);
        }
        g.submit(e);
        for k in (0..READS).filter(|&k| taken[k]) {
            let (s, ticket) = c.held[k];
            g.map_read(c.reads[k], 0, BACK_WORDS * 4, c.asked[s as usize].cells, &mut c.mapping[k], ticket);
            g.map_read(
                c.record_reads[k],
                0,
                ways::tree::RECORDS_BYTES,
                records + k as u32 * ways::tree::RECORDS_BYTES,
                &mut c.record_mapping[k],
                ticket,
            );
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

    /// The start and the cast where the frame casts, then the
    /// decor grown over the world where it moves: each a pass of its own
    /// over the cast's group, timed in its own span, so what each costs is
    /// read apart.
    fn dispatch(&self, g: &Gpu, encoder: u32, stamps: &mut [Stamp]) {
        let Some(c) = self.cast.as_ref() else { return };
        if self.kind == FrameKind::cast {
            let run = |stamp: &mut Stamp, built: &Built, [x, y]: [u32; 2]| {
                let pass = g.compute_pass(encoder, stamp.pass());
                g.set_bind_group(pass, 0, c.group, &[]);
                g.set_pipeline(pass, built.pipeline);
                g.dispatch(pass, x, y, 1);
                g.end(pass);
            };
            let w = compute::CAST_WORKGROUP;
            run(&mut stamps[SPAN_START], &c.start_built, [1, 1]);
            run(&mut stamps[SPAN_CAST], &c.built, [c.size[0].div_ceil(w[0]), c.size[1].div_ceil(w[1])]);
        }
        if self.grows() {
            let pass = g.compute_pass(encoder, stamps[SPAN_GROW].pass());
            c.grown.dispatch(g, pass);
            g.end(pass);
        }
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
        if self.kind == FrameKind::plain || self.group == 0 {
            return 0;
        }
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.group
    }
}

/// The cast's group 0 over `bound`: the eye, the window and its fold
/// (`joined_cs` writes it, the cast reads it), the tiles' frames, the
/// surface's rays, the squares' looks, the ways' pool, the service area's index, the overwrites and where its
/// rays start (`start_cs` writes it, `cast_cs` reads it).
fn cast_group(g: &Gpu, built: &Built, bound: [u32; 9]) -> u32 {
    let [eye, window, ground, rays, looks, shards, overwrites, starts, pool] = bound;
    let entries = [
        buffer_entry(0, eye),
        buffer_entry(1, window),
        buffer_entry(2, ground),
        view_entry(3, rays),
        buffer_entry(4, looks),
        buffer_entry(5, shards),
        buffer_entry(6, overwrites),
        buffer_entry(7, starts),
        buffer_entry(8, pool),
    ];
    g.bind_group(built.layouts[0], &entries)
}

/// A variant picker case: its own frame, drawn at one sample with no shadow.
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
    fn new(g: &Gpu, pipelines: &mut Pipelines, ao: &ScreenAo, air: [Entry; AIR_ENTRIES], align: u32) -> Case {
        let frame_buffer = g.buffer(word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let layout = frame_layout(g, pipelines);
        Case {
            frame_buffer,
            frames: FrameGroups::new(g, layout, frame_buffer, [ao.blank_entry(7); 2], air),
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
    query_set: u32,
    pipelines: Pipelines,
    frame_buffer: u32,
    frame: Frame,
    sky_buffer: u32,
    sky: [f32; SKY_FLOATS],
    sky_built: Built,
    sky_group: u32,
    ghost: [Built; 2],
    draws_buffer: u32,
    draws_bytes: u32,
    bind_group: u32,
    models: FrameGroups,
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
    /// The world image's whole input as the last image drew it (stage 2):
    /// the frame's, the sky's and the stars' words, the air's, the image's
    /// own textures, and what came or was drawn again without words.
    world: Read,
    /// What the frame draws from besides the world image, as last read: the
    /// draws and the blocks they read, the badges' sheet, the clouds' words
    /// and the cases. With the image's, the frame's whole input.
    frame_read: Read,
    /// What the occlusion was last made from: the frame's words, its own,
    /// the surface and the decor its prepass draws. The world image is made
    /// again for more than these, and the occlusion only where they moved.
    ao_read: Read,
    /// What lays the world image on a frame, and its group over the views
    /// it was made for.
    blit: Built,
    blit_group: u32,
    blit_views: [u32; 2],
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
        let clouds = CloudPass::new(g, &mut pipelines, frame_buffer, air);
        let mips = Mips::new(g, &mut pipelines, Format::rgba8unorm_srgb);
        let layers = Layers::new(g);
        let sky_built = pipelines.build(g, PIPELINE_SKY, g.canvas());
        let moon = ImageMap::new(g, MOON_TEXELS, MOON_TEXELS);
        let mut sky_entries = vec![buffer_entry(0, frame_buffer), buffer_entry(19, sky_buffer), view_entry(2, moon.view), sampler_entry(3, moon.sampler)];
        sky_entries.extend(air);
        let sky_group = g.bind_group(sky_built.layouts[0], &sky_entries);
        let draws_buffer = g.buffer(DRAWS_INITIAL * 4, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        let models = FrameGroups::new(g, frame_layout(g, &mut pipelines), frame_buffer, [ao.blank_entry(7); 2], air);
        let query_set = if device.timestamps { g.spans() } else { 0 };
        let mut r = Renderer {
            align: device.align,
            query_set,
            ghost: [pipelines.build(g, PIPELINE_GHOST_BOX, g.canvas()), pipelines.build(g, PIPELINE_GHOST_BOX_COPLANAR, g.canvas())],
            decor: DecorPass::new(g, &mut pipelines, device.align),
            impostor: ImpostorPass::new(g, &mut pipelines, device.align),
            skinned: SkinnedPass::new(g, &mut pipelines, device.align),
            puck: PuckPass::new(g, &mut pipelines, frame_buffer, air),
            badges: BadgePass::new(g, &mut pipelines, frame_buffer),
            debug: DebugPass::new(g, &mut pipelines, frame_buffer),
            screen: RaysPass::new(g, &mut pipelines),
            world: Read::default(),
            frame_read: Read::default(),
            ao_read: Read::default(),
            blit: pipelines.build(g, PIPELINE_BLIT, g.canvas()),
            blit_group: 0,
            blit_views: [0; 2],
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
    /// (paint.wgsl): the frame, the layers, the occlusion and the air.
    fn lit_entries(&self, frame: u32) -> Vec<Entry> {
        let mut e = vec![
            buffer_entry(0, frame),
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
        self.bind_group = g.bind_group(self.ghost[0].layouts[0], &e);
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
        put_ray(&mut self.sky, 0, &m, &eye);
        self.clouds
            .lay_out(&self.sky[..RAY_FLOATS], [eye_world[0], eye_world[1]], env, s[STAGE_AT::CLOUDS], reach * env[ENV_AIR + 1]);
        self.stars.lay_out(&m, env, width);
        self.sky[SKY_POLE..SKY_POLE + 3].copy_from_slice(&env[ENV_STARS + 8..ENV_STARS + 11]);
        self.sky[SKY_MAPPED] = if self.moon.loaded { 1.0 } else { 0.0 };
        for i in 0..3 {
            u[CAMERA_POS + i] = eye[i] as f32;
        }
        u[EDGE_STRENGTH] = s[STAGE_AT::EDGE_STRENGTH] as f32;
        u[CELL] = s[STAGE_AT::LOD_CELL] as f32;
        for (dir, color, at) in [(SUN_DIR, SUN_COLOR, ENV_SUN_LIGHT), (OTHER_DIR, OTHER_COLOR, ENV_MOON_LIGHT)] {
            u[dir..dir + 3].copy_from_slice(&env[at..at + 3]);
            u[color..color + 3].copy_from_slice(&env[at + LIGHT_COLOR..at + LIGHT_COLOR + 3]);
        }
        u[LOW_SPEC] = s[STAGE_AT::LOW_SPEC] as f32;
        u[EDGE_WIDTH] = s[STAGE_AT::EDGE_WIDTH] as f32;
        u[RIM_INTENSITY] = s[STAGE_AT::RIM_INTENSITY] as f32;
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

        self.atmosphere.lay_out(g, env);
    }

    /// A box is one instance shared by 36 vertices; a draw's first box is its first
    /// instance. Both vertex slots bind the same buffer; the record index rides
    /// on the first vertex.
    fn draw_boxes(&self, g: &Gpu, pass: u32, draws: &Draws, which: u32, own: u32) {
        let b = blocks();
        let mut bound = 0;
        for d in draws.of(which) {
            let (handle, box_cnt, first_box, uniform) = (d[0], d[1], d[2], d[3]);
            let buffer = b.get(handle).map(|(buffer, ..)| buffer);
            // A missing buffer is a fault in the list. Skipping it silently once drew no
            // ground at all with every gauge reading normal.
            let Some(buffer) = buffer else { xpute_kit::bug!(ENOTRECOVERABLE, handle, which) };
            if bound == 0 {
                g.set_pipeline(pass, own);
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

    /// A frame in three stages, each standing while what it reads stands:
    /// the cast's rays (stage 1, `RaysPass::prepare`); the world
    /// image, everything the world and its light make, into the surface's own
    /// colour and depth (stage 2, `world`); and the frame, the image laid on
    /// it whole and over it what moves on its own — the actors, whose clips
    /// play, the clouds, which the wind carries, the puck, the ghosts, the
    /// badges, the gizmos (stage 3, every frame).
    fn frame(&mut self, g: &Gpu, world: &mut Surface, input: FrameInput) -> (FrameKind, FrameStat) {
        let mut read = core::mem::take(&mut self.world);
        read.begin();
        let mut rest = core::mem::take(&mut self.frame_read);
        rest.begin();
        self.decor.set_models(input.decor);
        self.impostor.set_models(input.decor.iter().map(|&(mesh, image, extent)| (mesh, image.map(|i| i.0), extent)));
        for &(model, image) in input.skins {
            self.skinned.set_skin(g, model, image);
        }
        read.read(input.skins);
        read.mark(self.layers.step(g, &self.mips, input.pack));
        read.mark(self.moon.step(g, &self.mips, input.moon));
        read.mark(self.atmosphere.earth.step(g, &self.mips, input.earth));
        let cast = input.cast;
        self.screen.hand(g, &mut self.pipelines, input.ground.upload);
        let tiles = input.earth_tiles;
        read.mark(self.atmosphere.set_earth_tiles(g, tiles.window, tiles.layers, tiles.upload));
        rest.mark(self.badges.sync_sheet(g, &self.mips, input.billboards, input.text));
        let (records, uniforms, blocks_moved) = take_lists(Some(g), input.lists);
        rest.mark(blocks_moved);
        // Each list's writers write again only where it moved.
        let uniforms_moved = rest.read(uniforms);
        let lists_moved = rest.read(records) | uniforms_moved;
        rest.read(input.cases);

        targets(g, world);
        // How far the eye observes, as far as the clouds are drawn.
        let reach = cast.as_ref().map_or(0.0, |c| f32::from_bits(c.eye[eye::REACH]));
        self.lay_out(g, world.width, world.height, input.environment, reach);
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
            g.write_buffer(self.draws_buffer, 0, bytes(uniforms));
        }
        let mut order = core::mem::take(&mut self.order);
        let draws = Draws::sort(records.as_chunks::<DRAW_WORDS>().0, &mut order);
        // Each uniform written where the image's read finds it moved.
        if read.read(&self.frame) {
            g.write_buffer(self.frame_buffer, 0, bytes(&self.frame));
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
        read.read(&[world.world, world.world_depth, u32::from(self.screen_ao)]);
        let b = blocks();
        self.decor.prepare(g, &draws, uniforms, lists_moved, b);
        self.impostor.prepare(g, draws.of(PIPELINE_IMPOSTOR), uniforms, lists_moved, b);
        self.skinned.prepare(g, draws.of(PIPELINE_SKINNED), uniforms, lists_moved, b);
        self.puck.prepare_zone(g, &draws, b);
        self.badges.prepare(g, world.depth_view, (world.width, world.height), &draws, b);
        self.debug.prepare(g, &draws);
        let meshes: [u32; GROUPS as usize] = core::array::from_fn(|k| self.decor.model_of(g, k as u32, b).map_or(0, |m| m.2));
        self.screen.prepare(g, &mut self.pipelines, cast, &meshes, (world.rays_view, [world.width, world.height]));
        // The image draws the decor grown, so it reads whether that is on:
        // decor turned off grows nothing and casts no ray again, yet the
        // image stands without it.
        read.read(&[u32::from(self.screen.grown().is_some())]);
        self.decor.prepare_grown(g, self.screen.grown());
        self.impostor.prepare_grown(g, self.screen.grown());
        self.prepare_cases(g, &draws, uniforms, input.cases, lists_moved);
        let q = if input.timed { self.query_set } else { 0 };
        // A query no pass wrote holds an earlier frame's value, so which were written is passed along.
        let mut stamps: [Stamp; GPU_SPAN_NAMES.len()] = core::array::from_fn(|span| Stamp::of(q, span));
        // What outlives the frame is drawn and submitted ahead of it: from here on
        // the frame only describes itself.
        let ahead = g.encoder();
        let baked = self.impostor.bake(g, ahead, &mut self.decor, b);
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
        self.screen.read_back(g, input.ground.landed, input.ground.joined, input.ground.records);
        // The image stands where it read all as last time over rays that stood.
        let still = read.end() && self.screen.kind != FrameKind::cast && !self.screen.grows();
        if rest.read(self.clouds.words()) {
            self.clouds.write(g);
        }
        // The frame stands where the image does and the rest of what it
        // draws from read as last time: nothing is drawn, nothing submitted,
        // and the canvases keep the frame they show.
        if still && rest.end() && !gridded {
            self.world = read;
            self.frame_read = rest;
            let stat = self.stat(draws.records, world, input.counted, [0.0; GPU_SPAN_NAMES.len()]);
            self.order = order;
            return (FrameKind::stood, stat);
        }

        // The frame proper changes nothing that outlives it, so one too large
        // for the stream is dropped whole and the screen keeps the last.
        frame_begin();
        let encoder = g.encoder();
        self.screen.dispatch(g, encoder, &mut stamps);
        let a = &mut self.ao_read;
        a.begin();
        a.read(&self.frame);
        a.read(self.ao.words());
        a.read(&[world.width, world.height, u32::from(self.screen_ao), u32::from(self.screen.grown().is_some())]);
        a.mark(ao_remade || self.screen.grows());
        let ao_moved = !a.end();
        if !still {
            if self.screen_ao && ao_moved {
                let pass = self.ao.begin_prepass(g, encoder, &mut stamps[SPAN_PREPASS]);
                self.draw_opaque(g, pass, b, Stage::Prepass);
                g.end(pass);
                self.ao.run(g, encoder, &mut stamps[SPAN_AO]);
            }
            let pass = g.pass(encoder, &pass_desc(world.world_view, Load::Clear, world.world_depth_view, Load::Clear, 0.0, stamps[SPAN_WORLD].pass()));
            self.draw_opaque(g, pass, b, Stage::Lit);
            let mut bound = self.screen.draw(g, pass);
            // Last among opaque pipelines: alpha-tested fragments they discard were already filled.
            self.impostor.draw_grown(g, pass, self.screen.grown(), GROUPS, (self.models.frame, &mut bound));
            // At the far plane after the opaque world, so only empty pixels run the sky shader.
            g.set_pipeline(pass, self.sky_built.pipeline);
            g.set_bind_group(pass, 0, self.sky_group, &[]);
            g.draw(pass, 3, 1, 0, 0);
            self.stars.draw(g, pass);
            g.end(pass);
        }
        let canvas_view = g.current(world.handle);
        let pass = g.pass(encoder, &pass_desc(canvas_view, Load::Clear, world.depth_view, Load::Clear, 0.0, stamps[SPAN_OVERLAY].pass()));
        let views = [world.world_view, world.world_depth_view];
        if views != self.blit_views {
            release(self.blit_group);
            self.blit_group = g.bind_group(self.blit.layouts[0], &[view_entry(0, views[0]), view_entry(1, views[1])]);
            self.blit_views = views;
        }
        g.set_pipeline(pass, self.blit.pipeline);
        g.set_bind_group(pass, 0, self.blit_group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        // The actors, whose clips play, and the clouds the kernel lists, the
        // cumulus, which the wind carries: drawn over the image against its
        // depth, as a moving object is over a cached world, their occlusion
        // open.
        let mut bound = 0;
        let over = (self.models.over, &mut bound);
        self.skinned.draw(g, pass, draws.of(PIPELINE_SKINNED), b, Stage::Lit, (over.0, &mut *over.1));
        self.decor.draw(g, pass, &draws, b, Stage::Lit, (over.0, &mut *over.1));
        self.impostor.draw(g, pass, draws.of(PIPELINE_IMPOSTOR), b, over);
        self.clouds.draw(g, pass);
        self.puck.draw_beacon(g, pass, &draws, uniforms, lists_moved);
        self.puck.draw_zone(g, pass, &draws, uniforms, lists_moved, b);
        self.draw_boxes(g, pass, &draws, PIPELINE_GHOST_BOX, self.ghost[0].pipeline);
        self.draw_boxes(g, pass, &draws, PIPELINE_GHOST_BOX_COPLANAR, self.ghost[1].pipeline);
        let mut bound = 0;
        self.decor.draw_ghosts(g, pass, &draws, b, (self.models.frame, &mut bound));
        g.end(pass);
        self.badges.draw(g, encoder, canvas_view, world.depth_view, &draws, uniforms, b, &mut stamps[SPAN_BADGES]);
        self.debug.draw(g, encoder, canvas_view, world.depth_view, &draws, uniforms, lists_moved, b, &mut stamps[SPAN_DEBUG]);
        self.draw_cases(g, encoder, Self::cases(&draws, input.cases), uniforms, blocks_moved || tables || exposed, &mut stamps[SPAN_CASES]);
        let written = stamps.iter().enumerate().filter(|(_, s)| s.written()).fold(0, |m, (k, _)| m | 1 << k);
        g.submit_frame(encoder, written);
        release(canvas_view);
        let mut gpu_spans = [0.0; GPU_SPAN_NAMES.len()];
        let gpu_read = q != 0 && g.gpu_landed();
        if q != 0 {
            gpu_spans = core::array::from_fn(|k| g.gpu_ms(k as u32));
        }
        let skipped = !frame_commit();
        if skipped {
            REDRAW.set(true);
            // Its cast and its image were dropped with it: the rays and the
            // image hold older ones.
            if let Some(c) = self.screen.cast.as_mut() {
                c.read.forget();
                c.grown.dropped();
            }
            for case in self.cases.iter_mut().flatten() {
                case.read.forget();
            }
            read.forget();
            rest.forget();
            self.ao_read.forget();
        } else if self.screen.grows() {
            // Drawn: what its decor marked by is done with.
            if let Some(c) = self.screen.cast.as_mut() {
                c.grown.drawn_frame();
            }
        }
        self.world = read;
        rest.end();
        self.frame_read = rest;
        let kind = match (skipped, still) {
            (true, _) => FrameKind::dropped,
            (false, true) => FrameKind::still,
            (false, false) => self.screen.kind,
        };
        let mut stat = self.stat(draws.records, world, input.counted, gpu_spans);
        stat.gpu_read = gpu_read;
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
                self.cases[slot] = Some(Case::new(g, &mut self.pipelines, &self.ao, self.atmosphere.entries(), self.align));
            }
            let case = self.cases[slot].as_mut().or_bug(Errno::ENOTRECOVERABLE);
            case.skinned.set_skin(g, model, image);
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
    fn stat(&self, draws: &[[u32; DRAW_WORDS]], world: &Surface, counted: bool, gpu_spans: [f64; GPU_SPAN_NAMES.len()]) -> FrameStat {
        let b = blocks();
        let mut census: DrawCensus = [[0.0; 2]; DRAW_PIPELINES.len()];
        for d in draws.iter().filter(|_| counted) {
            let tris = match d[4] {
                PIPELINE_GHOST_BOX | PIPELINE_GHOST_BOX_COPLANAR => d[1] * 12,
                PIPELINE_DECOR | PIPELINE_GHOST_DECOR => self.decor.triangles_of(d[1], b) * d[2],
                PIPELINE_IMPOSTOR => 2 * d[2],
                PIPELINE_SKINNED => b.get(d[0]).map_or(0, |(_, _, index_cnt)| index_cnt / 3),
                PIPELINE_PUCK_ZONE => 12 * d[1],
                PIPELINE_PUCK_BEACON => 8,
                PIPELINE_BADGE => 2 * d[1],
                // Two a segment, and the faces'.
                PIPELINE_DEBUG => d[1] + d[2] / 3,
                _ => 0,
            };
            let c = &mut census[census_slot(d[4])];
            c[0] += 1.0;
            c[1] += tris as f64;
        }
        let px = world.width as f64 * world.height as f64;
        FrameStat {
            gpu_read: false,
            blocks: b.count as f64,
            block_bytes: b.bytes as f64,
            // The depth targets at the world's size: its own, one f32 a pixel, AO's and the badges' regions'.
            depth_mb: round((px * 4.0 + self.ao.depth_bytes() + self.badges.depth_bytes()) / MB),
            layers_mb: round(self.layers.bytes as f64 / MB),
            world_px: px,
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
    // The world's alone draws over an image of it; a case draws straight.
    (s.world, s.world_view, s.world_depth, s.world_depth_view, s.rays, s.rays_view) = (0, 0, 0, 0, 0, 0);
    if s.role == surface_role::WORLD {
        s.world = g.texture(width, height, 1, g.canvas(), 1, usage);
        s.world_view = g.view(s.world, false, 0, 1, 0, 1);
        s.world_depth = g.texture(width, height, 1, Format::depth32float, 1, usage);
        s.world_depth_view = g.view(s.world_depth, false, 0, 1, 0, 1);
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
    let Some(d) = cast.as_ref().and_then(|c| c.decor.as_ref()) else { return };
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
