// pxl-kernel/layer/decor/cell.rs

//! Decor as a payload of the chunks the terrain materializes: each mounted
//! chunk holds the decor standing inside it, built over its level's page and
//! the object store, and drawn while the chunk is. A block is immutable once
//! built; how each instance is drawn (`present`) is decided per frame and
//! rebuilds nothing.

use std::collections::HashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource};
use pxl_spec::procedural::Frame;
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND};
use pxl_spec::space::v3::{len3, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;

use crate::control::camera::controller::units_per_px_at;
use crate::global::Global;
use crate::layer::decor::build::{decor_gone_px, decor_sprite_px, variant_extent, DecorBuild, DecorHide, Ground, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::decor::kind::{Kind, KINDS};
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{object_col_revision, Store};
use crate::shell::setting::store::settings_store;
use crate::world::change::request_world_tick;
use crate::world::chunk::build::Builder;
use crate::world::chunk::mesh_slot::OCCUPANCY;
use crate::world::chunk::table::Scheduler;
use crate::world::observation::{eye, observation};
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use pxl_spec::bake::format::Terrain;
use xpute_runtime::mem::heap::free;

struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    /// Byte offset of the index list past the instances, rewritten each frame by `present`.
    index: u32,
    /// The lowest and highest instance, in units.
    heights: [f32; 2],
    sources: Sources,
    /// Its chunk is drawn and in the frustum.
    drawn: bool,
}

impl Block {
    fn words(&self) -> &'static [u32] {
        // SAFETY: the chunk's block, `bytes` long, until given back on a later turn.
        unsafe { core::slice::from_raw_parts(ptr_at::<u32>(self.at as usize), self.bytes as usize / 4) }
    }
}

/// What a population depends on beyond its page and the seed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    /// The level of the page it is built from, coarser until its own comes.
    read_from: u32,
    revision: u32,
    shown: bool,
    hide: Option<DecorHide>,
}

#[derive(Default)]
struct Cells {
    blocks: HashMap<Chunk, Block>,
    scratch: Option<DecorScratch>,
    sample: ChunkSample,
    cols: Vec<IVec>,
    placed: Vec<Placed>,
    folded: HashMap<[i64; 3], u32>,
    /// Kept so a settled turn asks the heap for nothing.
    mounted: Vec<(Chunk, bool, bool)>,
    gone: Vec<Chunk>,
    sprites: Vec<u32>,
}

static CELLS: Global<Cells> = Global::new();

/// The scattered decor the eraser is over, drawn hidden as a preview.
static SCATTERED_HIDE: Global<Option<[i64; 3]>> = Global::new();

pub fn scattered_hide() -> Option<[i64; 3]> {
    *SCATTERED_HIDE.get(|| None)
}

pub fn set_scattered_hide(cell: Option<[i64; 3]>) {
    let held = SCATTERED_HIDE.get(|| None);
    if *held != cell {
        *held = cell;
        request_world_tick();
    }
}

fn cells() -> &'static mut Cells {
    CELLS.get(Cells::default)
}

/// One a mounted chunk.
pub const BLOCKS_MAX: usize = OCCUPANCY;

const BUILDS_PER_FRAME: u32 = 4;

/// What a chunk is admitted by before its block says what it holds.
fn largest_possible(kinds: &[&Kind]) -> f64 {
    let extent = kinds.iter().map(|k| k.extent()).fold(0.0, f64::max);
    extent * (1.0f64 + SCALE_JITTER).max(DECOR_SIZE)
}

/// Whether anything a chunk could hold is in reach and over the sprite's floor.
fn may_show(chunk: Chunk, largest: f64) -> bool {
    let Some(o) = observation() else { return false };
    let region = chunk.region();
    let near = region.nearest(o.eye);
    let d = len3(&[near[0] - o.eye[0], near[1] - o.eye[1], near[2] - o.eye[2]]).max(1e-3);
    d <= o.far && largest / units_per_px_at(d) >= decor_sprite_px()
}

/// None where the heap refused room to list the columns.
fn sources_of(chunk: Chunk, store: &Store, cols: &mut Vec<IVec>, hide: Option<DecorHide>) -> Option<Sources> {
    cols.clear();
    store.cols_under(chunk.column(), cols).ok()?;
    let revision = cols.iter().fold(0u32, |r, &c| r.wrapping_mul(31).wrapping_add(object_col_revision(c)));
    let (x0, y0, side) = chunk.rect();
    let inside = |x: i64, y: i64| (x0..x0 + side).contains(&(x as f64)) && (y0..y0 + side).contains(&(y as f64));
    let hide = hide.filter(|h| match *h {
        DecorHide::Placed(at) => {
            let o = FROM_L2_IDX(at.col_idx);
            inside(ivec::X(o), ivec::Y(o))
        }
        DecorHide::Scattered([x, y, _]) => chunk.level() == 0 && inside(x, y),
    });
    Some(Sources {
        read_from: Terrain::pyramid().paging().drawn_from(chunk.page())?.level,
        revision,
        shown: settings_store().decor,
        hide,
    })
}

fn build(c: &mut Cells, chunk: Chunk, kinds: &[&Kind], store: &Store, draw: &mut DrawList, sources: Sources) -> Option<Block> {
    let pages = Terrain::pyramid();
    // The page the terrain draws the chunk from, read over the chunk of its level.
    let src = pages.paging().drawn_from(chunk.page())?;
    let page = pages.paging().get(src)?;
    let mut over = chunk;
    while over.level() < src.level {
        over = over.parent()?;
    }
    let level = over.level();
    let (cx0, cy0, _) = chunk.rect();
    let (x0, y0, _) = over.rect();
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (tx0, ty0) = over.in_page();
    let view = Builder::page_view(page);
    if sources.shown {
        let around = Around {
            own: PageSource {
                page: view,
                frame: Frame::of(src),
                origin: [0, 0],
                n: 1,
            },
            beside: [None; 8],
        };
        c.sample.read(&around, [tx0 as i64, ty0 as i64]).ok()?;
    }
    c.placed.clear();
    c.placed.try_reserve(c.cols.len()).ok()?;
    c.placed.extend(c.cols.iter().map(|&col_idx| {
        let o = FROM_L2_IDX(col_idx);
        Placed {
            col_idx,
            ox: ivec::X(o) as f64 - cx0,
            oy: ivec::Y(o) as f64 - cy0,
            hittable: chunk.level() == 0,
        }
    }));
    let target = GROUND.scale(level);
    let (cx, cy, span) = (target.index(x0), target.index(y0), CHUNK_CELLS as i64);
    match level > 0 && sources.shown {
        true => store.fold_at(target, [cx, cy], [cx + span, cy + span], sources.hide.and_then(DecorHide::placed), &mut c.folded).ok()?,
        false => c.folded.clear(),
    }
    let ground = Ground {
        page: &view,
        sample: &c.sample,
        level,
        x0,
        y0,
        tx0,
        ty0,
        col_idx: (level == 0).then(|| L2_IDX(ivec::gather(x0, y0, 0))),
        folded: (level > 0).then_some(&c.folded),
    };
    let scratch = c.scratch.get_or_insert_with(DecorScratch::default);
    let step = GROUND.scale(0).count_in(target.max(GROUND.scale(0)));
    let built = DecorBuild::build(scratch, kinds, Some(&ground), &c.placed, sources.hide, store, step, sources.shown, chunk.region())?;
    let index = built.bytes();
    let heights = built.heights();
    let instances = built.instances() as u32;
    let (at, bytes) = alloc(index + instances * 4)?;
    // SAFETY: a block of the bytes the build asked for, the chunk's until given back.
    unsafe { built.write(ptr_at::<u8>(at)) };
    let handle = draw.upload(at as u32, bytes, 0, 0);
    Some(Block {
        at: at as u32,
        bytes,
        handle,
        index,
        heights,
        sources,
        drawn: false,
    })
}

fn release(block: Block, draw: &mut DrawList) {
    draw.free(block.handle);
    // SAFETY: the chunk's block, not given back yet; nothing reads it past this turn.
    unsafe { free((block.at as usize, block.bytes)) };
}

/// Builds the decor of the chunks the terrain has mounted, a few a frame, and
/// lets go of what an unmounted chunk held.
pub fn step(kinds: &[&Kind], store: &Store, sched: &mut Scheduler, hide: Option<DecorHide>, frame_eye: V3) {
    let c = cells();
    let mut mounted = core::mem::take(&mut c.mounted);
    sched.mounted(&mut mounted);
    mounted.sort_unstable_by_key(|m| m.0);
    let largest = largest_possible(kinds);
    let held = |k: &Chunk| mounted.binary_search_by_key(k, |m| m.0).is_ok_and(|i| may_show(mounted[i].0, largest));
    c.gone.clear();
    c.gone.extend(c.blocks.keys().copied().filter(|k| !held(k)));
    for k in c.gone.drain(..) {
        if let Some(b) = c.blocks.remove(&k) {
            release(b, &mut sched.draw);
        }
    }
    let mut built = 0;
    for &(chunk, _, _) in &mounted {
        if built >= BUILDS_PER_FRAME {
            break;
        }
        if !may_show(chunk, largest) {
            continue;
        }
        let Some(sources) = sources_of(chunk, store, &mut c.cols, hide) else { continue };
        if c.blocks.get(&chunk).is_some_and(|b| b.sources == sources) {
            continue;
        }
        if let Some(b) = build(c, chunk, kinds, store, &mut sched.draw, sources) {
            if let Some(old) = c.blocks.insert(chunk, b) {
                release(old, &mut sched.draw);
            }
            built += 1;
        }
    }
    for &(chunk, drawn, seen) in &mounted {
        if let Some(b) = c.blocks.get_mut(&chunk) {
            b.drawn = drawn && seen;
        }
    }
    c.mounted = mounted;
    present(c, &mut sched.draw, frame_eye);
}

/// Per frame, each instance as model, sprite or nothing by its projected
/// size; a group surely one or the other over the chunk's nearest and farthest
/// distance is settled whole. Models come first in a group's index list, then
/// sprites; the list is re-uploaded only where it changed.
fn present(c: &mut Cells, draw: &mut DrawList, frame_eye: V3) {
    let Some(eye) = eye() else { return };
    let model_px = decor_gone_px();
    let sprite_px = decor_sprite_px();
    let sprites = &mut c.sprites;
    for (&chunk, block) in c.blocks.iter().filter(|(_, b)| b.drawn) {
        let (x0, y0, side) = chunk.rect();
        let translation = DrawList::translation([x0, y0, 0.0], frame_eye);
        let [zlo, zhi] = block.heights.map(f64::from);
        let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
        let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
        let nearest = len3(&[gap(x0, x0 + side, eye[0]), gap(y0, y0 + side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
        let farthest = len3(&[reach(x0, x0 + side, eye[0]), reach(y0, y0 + side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
        // SAFETY: the chunk's block, `bytes` long, its own to write past `index`.
        let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block.at as usize), block.bytes as usize / 4) };
        let (lanes_of, list) = words.split_at_mut(block.index as usize / 4);
        let mut extents = KINDS.iter().flat_map(|k| k.variants.iter().map(variant_extent));
        let (mut at, mut changed) = (0usize, false);
        for (group, (offset, count)) in decor_groups(lanes_of).enumerate() {
            let extent = extents.next().unwrap_or(0.0);
            let lane = lanes(offset, count);
            let first = at;
            sprites.clear();
            // Scattered scales are within the jitter; a placed one's is 1.
            let (small, large) = (1.0 - SCALE_JITTER, 1.0 + SCALE_JITTER);
            let gone = extent * large / units_per_px_at(nearest) < sprite_px;
            let modeled = extent * small / units_per_px_at(farthest) >= model_px;
            for i in 0..if gone { 0 } else { count as usize } {
                if (lanes_of[lane.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                    continue;
                }
                if modeled {
                    changed |= list[at] != i as u32;
                    list[at] = i as u32;
                    at += 1;
                    continue;
                }
                let f = |l: usize| f32::from_bits(lanes_of[lane.f32_at(l, i)]) as f64;
                let d = len3(&[x0 + f(0) - eye[0], y0 + f(1) - eye[1], f(2) - eye[2]]).max(1e-3);
                let px = extent * f(4) / units_per_px_at(d);
                if px >= model_px {
                    changed |= list[at] != i as u32;
                    list[at] = i as u32;
                    at += 1;
                } else if px >= sprite_px {
                    sprites.push(i as u32);
                }
            }
            let models = (at - first) as u32;
            changed |= list[at..at + sprites.len()] != sprites[..];
            list[at..at + sprites.len()].copy_from_slice(sprites);
            at += sprites.len();
            let index = block.index / 4 + first as u32;
            let group = group as u32;
            if models > 0 {
                draw.draw_instances(PIPELINE_DECOR, block.handle, group, models, [offset, count, index], translation, 1.0);
            }
            if !sprites.is_empty() {
                draw.draw_instances(PIPELINE_IMPOSTOR, block.handle, group, sprites.len() as u32, [offset, count, index + models], translation, 1.0);
            }
        }
        if changed {
            draw.rewrite_part(block.handle, block.at, block.index, at as u32 * 4);
        }
    }
}

/// The drawn chunks' decor: the chunk, its corner and its block.
pub fn shown() -> impl Iterator<Item = (Chunk, f64, f64, &'static [u32])> {
    cells().blocks.iter().filter(|(_, b)| b.drawn).map(|(&chunk, b)| {
        let (x0, y0, _) = chunk.rect();
        (chunk, x0, y0, b.words())
    })
}

pub fn lanes(offset: u32, count: u32) -> GroupLanes {
    GroupLanes {
        first: offset as usize,
        count: count as usize,
    }
}

pub fn bytes() -> u32 {
    cells().blocks.values().map(|b| b.bytes).sum()
}

pub fn counts() -> (u32, u32) {
    let c = cells();
    let instances = c.blocks.values().map(|b| decor_groups(b.words()).map(|(_, n)| n).sum::<u32>()).sum();
    (c.blocks.len() as u32, instances)
}
