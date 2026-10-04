// pxl-kernel/layer/decor/cell.rs

//! Decor as a payload of the chunks the terrain materializes: each mounted
//! chunk holds the decor standing inside it, built over its level's page and
//! the object store, and drawn while the chunk is. A block is immutable once
//! built; how each instance is drawn (`present`) is decided per frame and
//! rebuilds nothing.

use std::collections::{HashMap, HashSet};

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource};
use pxl_spec::procedural::Frame;
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND};
use pxl_spec::space::v3::{len3, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::sched::tick::FrameBudget;

use crate::control::camera::controller::units_per_px_at;
use crate::global::Global;
use crate::layer::decor::build::{decor_gone_px, decor_sprite_px, variant_extent, DecorBuild, DecorHide, Ground, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::decor::kind::{Kind, KINDS};
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{object_col_revision, Store};
use crate::shell::setting::store::settings_store;
use crate::world::chunk::build::Builder;
use crate::world::observation::{eye, frustum_of_view};
use crate::world::paging::floor;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use pxl_spec::bake::format::Terrain;
use xpute_runtime::mem::heap::free;

struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    /// The byte the index list starts at, past the instances: a word an
    /// instance, rewritten a frame at a time by `present`.
    index: u32,
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
    /// The cells the eye wants, as last solved, in cell order: what is
    /// drawn moves to it one whole replacement at a time (`step`).
    frontier: Vec<DecorCell>,
    /// The terrain pages the frontier scatters over, each once, in order.
    pages: Vec<LevelPage>,
    blocks: HashMap<DecorCell, Block>,
    scratch: Option<DecorScratch>,
    sample: ChunkSample,
    cols: Vec<IVec>,
    placed: Vec<Placed>,
    folded: HashMap<[i64; 3], u32>,
    /// Kept so a settled turn asks the heap for nothing.
    mounted: Vec<(Chunk, bool, bool)>,
    gone: Vec<Chunk>,
    pages: Vec<LevelPage>,
    sprites: Vec<u32>,
}

static CELLS: Global<Cells> = Global::new();

fn cells() -> &'static mut Cells {
    CELLS.get(Cells::default)
}

/// One a mounted chunk.
pub const BLOCKS_MAX: usize = OCCUPANCY;

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
    }
}

/// Builds cell `cell`'s block where its page is resident; none where it is
/// not yet, or the heap refused the block.
fn build(c: &mut Cells, cell: DecorCell, kinds: &[Kind], store: &Store, draw: &mut DrawList, sources: Sources) -> Option<Block> {
    let pages = Terrain::pyramid();
    let page = pages.get(cell.page())?;
    let level = cell.level;
    let (x0, y0, _) = cell.rect();
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (tx0, ty0) = cell.chunk().in_page();
    let view = Builder::page_view(page);
    if sources.shown {
        let around = Around {
            own: PageSource { page: view, origin: [0, 0], n: 1 },
            beside: [None; 8],
        };
        c.sample.read(&around, [tx0 as i64, ty0 as i64], hidden_ground(settings_store().structures)).ok()?;
    }
    c.placed.clear();
    c.placed.try_reserve(c.cols.len()).ok()?;
    c.placed.extend(c.cols.iter().map(|&col_idx| {
        let o = FROM_L2_IDX(col_idx);
        Placed {
            col_idx,
            ox: (ivec::X(o) - x0) as f64,
            oy: (ivec::Y(o) - y0) as f64,
            hittable: level == 0,
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
    // A cell is a whole column: every instance over it, at any height.
    let step = GROUND.scale(0).count_in(target.max(GROUND.scale(0)));
    let built = DecorBuild::build(scratch, kinds, Some(&ground), &c.placed, sources.hide, store, step, sources.shown)?;
    let index = built.bytes();
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
        sources,
        drawn: false,
    })
}

fn release(block: Block, draw: &mut DrawList) {
    draw.free(block.handle);
    // SAFETY: the chunk's block, not given back yet; nothing reads it past this turn.
    unsafe { free((block.at as usize, block.bytes)) };
}

/// Builds the decor of the chunks the terrain has mounted, within what is
/// left of the turn as the chunks' builds are, and lets go of what an
/// unmounted chunk held.
pub fn step(kinds: &[&Kind], store: &Store, sched: &mut Scheduler, hide: Option<DecorHide>, frame_eye: V3, budget: &FrameBudget) {
    let c = cells();
    let mut frontier = core::mem::take(&mut c.frontier);
    solve(kinds, &mut c.stack, &mut frontier);
    xpute_core::ensure!(frontier.len() <= FRONTIER_MAX, ENOTRECOVERABLE, frontier.len());
    let top = floor::level().unwrap_or(0);
    let ancestors = move |cell: DecorCell| core::iter::successors(Some(cell), move |a| (a.level < top).then(|| a.parent())).skip(1);
    let pages = Terrain::pyramid();

    // What decor builds is admitted against `BLOCKS_MAX`. A cell built over
    // cells held under it — a coarsening — lets those go once it is in, so
    // it comes first and is admitted at the budget too: the turn ends with
    // fewer blocks than it began. Any other new cell is admitted only below
    // the budget, and a rebuild of a cell held takes the place of its old
    // block. A cell not admitted waits, and what stands in for it stays
    // drawn: the budget slows a change of view, never opens a hole in it.
    c.over.clear();
    c.over.extend(c.blocks.keys().flat_map(|&k| ancestors(k)));
    c.over.sort_unstable();
    c.over.dedup();
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
        }
    }
    for &(chunk, drawn, seen) in &mounted {
        if let Some(b) = c.blocks.get_mut(&chunk) {
            b.drawn = drawn && seen;
        }
    }
    let under = &c.under;
    let kept = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| wanted(&cell) || under.get(&cell) == Some(&false) || ancestors(cell).find(wanted).is_some_and(|a| !ready(a, blocks));
    c.cells.clear();
    c.cells.extend(c.blocks.keys().copied().filter(|&k| !kept(k, &c.blocks)));
    for k in c.cells.drain(..) {
        if let Some(b) = c.blocks.remove(&k) {
            release(b, draw);
        }
    }
    // A held cell still waiting on what refines it covers its region, so
    // nothing under it is drawn.
    c.covers.clear();
    c.covers.extend(c.blocks.keys().copied().filter(|k| under.get(k) == Some(&false) && !wanted(k)));
    c.covers.sort_unstable();
    for (&cell, b) in c.blocks.iter_mut() {
        b.drawn = !ancestors(cell).any(|a| c.covers.binary_search(&a).is_ok());
    }
    c.pages.clear();
    c.pages.extend(frontier.iter().map(|c| c.page()));
    c.pages.sort_unstable();
    c.pages.dedup();
    c.frontier = frontier;
    present(c, draw, frame_eye);
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
            let mut sprites = Vec::new();
            for i in 0..count as usize {
                if (words[lane.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                    continue;
                }
                let f = |l: usize| f32::from_bits(words[lane.f32_at(l, i)]) as f64;
                let d = (x0 + f(0) - eye[0]).hypot(y0 + f(1) - eye[1]).hypot(f(2) - eye[2]).max(1e-3);
                let px = extent * f(4) / units_per_px_at(d);
                if px >= model_px {
                    words[at] = i as u32;
                    at += 1;
                } else if px >= sprite_px {
                    sprites.push(i as u32);
                }
            }
            let models = (at - first) as u32;
            words[at..at + sprites.len()].copy_from_slice(&sprites);
            at += sprites.len();
            let group = group as u32;
            if models > 0 {
                draw.draw_instances(PIPELINE_DECOR, block.handle, group, models, [offset, count, first as u32], translation, 1.0);
            }
            if !sprites.is_empty() {
                draw.draw_instances(PIPELINE_IMPOSTOR, block.handle, group, sprites.len() as u32, [offset, count, first as u32 + models], translation, 1.0);
            }
        }
        if at > index as usize {
            draw.rewrite_part(block.handle, block.at, block.index, (at - index as usize) as u32 * 4);
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

/// A group's lanes in a cell's block, as a hit reads them.
pub fn lanes(offset: u32, count: u32) -> GroupLanes {
    GroupLanes {
        first: offset as usize,
        count: count as usize,
    }
}

/// The bytes the cells' blocks hold in the heap.
pub fn bytes() -> u32 {
    cells().blocks.values().map(|b| b.bytes).sum()
}

/// The cells holding a block, and the instances their blocks hold.
pub fn counts() -> (u32, u32) {
    let c = cells();
    let instances = c.blocks.values().map(|b| decor_groups(b.words()).map(|(_, n)| n).sum::<u32>()).sum();
    (c.blocks.len() as u32, instances)
}

/// The terrain pages the frontier's cells scatter over, each once: what
/// decor reads of the ground, asked for by decor (world/pyramid.rs
/// `step`'s readers) rather than left to what the terrain draws.
pub fn pages() -> Vec<LevelPage> {
    let mut out: Vec<LevelPage> = cells().frontier.iter().map(|c| c.page()).collect();
    out.sort_unstable();
    out.dedup();
    out
}
