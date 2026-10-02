// pxl-kernel/layer/decor/cell.rs

//! Decor as its own layer. A decor cell is a column of a level's chunks; the
//! frontier is the cells the eye wants, refined by `Tau::level` and dropped
//! where the largest possible instance is below the sprite's floor or out of
//! view. A block is immutable once built; how each instance is drawn
//! (`present`) is decided per frame and rebuilds nothing.

use std::collections::HashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource};
use pxl_spec::space::hierarchy::{child_of, parent_of};
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::lod::fault_radius;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, GROUND, TOP_LEVEL};
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;

use crate::control::camera::controller::units_per_px_at;
use crate::global::Global;
use crate::layer::decor::asset_store::decor_bundle_assets;
use crate::layer::decor::build::{decor_gone_px, variant_extent, DecorBuild, DecorHide, Ground, Kind, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{object_col_revision, Store};
use crate::param::IMPOSTOR_GONE_CELLS;
use crate::shell::setting::store::{hidden_ground, settings_store};
use crate::world::change::request_world_tick;
use crate::world::chunk::build::Builder;
use crate::world::observation::{eye, frustum_of_view};
use crate::world::paging::floor;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use crate::world::quality;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::lod::target_cell_px;
use xpute_runtime::mem::heap::free;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DecorCell {
    pub level: u32,
    pub at: [i64; 2],
}

impl DecorCell {
    fn chunk(self) -> Chunk {
        Chunk::at(self.level, [self.at[0], self.at[1], 0])
    }

    /// Corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        self.chunk().rect()
    }

    fn page(self) -> LevelPage {
        self.chunk().page()
    }

    fn parent(self) -> DecorCell {
        DecorCell {
            level: self.level + 1,
            at: parent_of(self.at),
        }
    }

    fn children(self) -> [DecorCell; 4] {
        core::array::from_fn(|slot| DecorCell {
            level: self.level - 1,
            at: child_of(self.at, slot),
        })
    }
}

struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    /// Byte offset of the index list past the instances, rewritten each frame by `present`.
    index: u32,
    sources: Sources,
    /// Held hidden while it waits on the rest of a replacement (`step`).
    drawn: bool,
}

/// What a population depends on beyond its page and the seed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    revision: u32,
    shown: bool,
    hide: Option<DecorHide>,
}

#[derive(Default)]
struct Cells {
    frontier: Vec<DecorCell>,
    pages: Vec<LevelPage>,
    blocks: HashMap<DecorCell, Block>,
    scratch: Option<DecorScratch>,
    sample: ChunkSample,
    cols: Vec<IVec>,
    placed: Vec<Placed>,
    folded: HashMap<[i64; 3], u32>,
    /// Kept so a settled turn asks the heap for nothing.
    stack: Vec<DecorCell>,
    under: HashMap<DecorCell, bool>,
    cells: Vec<DecorCell>,
    covers: Vec<DecorCell>,
    over: Vec<DecorCell>,
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

/// Upper bound on a frontier: the top's (2n + 1)² roots with
/// n = ⌈fault_radius(top) / side⌉ + 1, plus, for each of `TOP_LEVEL` levels
/// under it, the children of the parents a disc of `fault_radius(L)` meets
/// (⌊2r / side⌋ + 2 a side, two children each). Split distance and chunk
/// grow by the same factor a level, so that count is level-independent.
pub const FRONTIER_MAX: usize = {
    let side = chunk_span_of(TOP_LEVEL) as u64;
    let n = (fault_radius(TOP_LEVEL) as u64).div_ceil(side) + 1;
    let top = (2 * n + 1) * (2 * n + 1);
    let across = 2 * ((2 * fault_radius(0) as u64) / chunk_span_of(1) as u64 + 2);
    (top + TOP_LEVEL as u64 * across * across) as usize
};

/// A residency budget kept by admission: a frontier wanted plus a frontier
/// drawn standing in for it.
pub const BLOCKS_MAX: usize = 2 * FRONTIER_MAX;

const BUILDS_PER_FRAME: u32 = 4;

/// What a cell is admitted by before its block says what it holds.
fn largest_possible(kinds: &[Kind]) -> f64 {
    let extent = kinds.iter().map(|k| k.extent).fold(0.0, f64::max);
    extent * (1.0f64 + SCALE_JITTER).max(DECOR_SIZE)
}

fn nearest(cell: DecorCell, eye: V3, ground: f64) -> f64 {
    let (x0, y0, side) = cell.rect();
    let dx = (x0 - eye[0]).max(eye[0] - (x0 + side)).max(0.0);
    let dy = (y0 - eye[1]).max(eye[1] - (y0 + side)).max(0.0);
    dx.hypot(dy).hypot(eye[2] - ground)
}

/// Kept within the view's reach, in the frustum, and where its largest
/// possible instance covers the sprite's floor.
fn solve(kinds: &[Kind], stack: &mut Vec<DecorCell>, out: &mut Vec<DecorCell>) {
    out.clear();
    stack.clear();
    let (Some(eye), Some(top)) = (eye(), floor::level()) else { return };
    let tau = quality::effective().tau();
    let frustum = frustum_of_view();
    let largest = largest_possible(kinds);
    let floor_px = target_cell_px() * crate::param::IMPOSTOR_GONE_CELLS;
    let view_reach = crate::layer::environment::fog::view_reach();
    let reach = fault_radius(top).min(view_reach);
    let side = chunk_span_of(top);
    let (cx, cy) = ((eye[0] / side).floor() as i64, (eye[1] / side).floor() as i64);
    let n = (reach / side).ceil() as i64 + 1;
    let ground = crate::layer::environment::fog::eye_ground();
    for y in cy - n..=cy + n {
        for x in cx - n..=cx + n {
            stack.push(DecorCell { level: top, at: [x, y] });
        }
    }
    while let Some(cell) = stack.pop() {
        let d = nearest(cell, eye, ground).max(1e-3);
        if d > view_reach || largest / units_per_px_at(d) < floor_px {
            continue;
        }
        let (x0, y0, s) = cell.rect();
        if !frustum.sees([x0 + s / 2.0, y0 + s / 2.0, ground], s * 0.75 + largest) {
            continue;
        }
        if cell.level > 0 && (tau.level(d).max(0) as u32) < cell.level {
            stack.extend(cell.children());
            continue;
        }
        out.push(cell);
    }
    out.sort_unstable();
}

/// None where the heap refused room to list the columns.
fn sources_of(cell: DecorCell, store: &Store, cols: &mut Vec<IVec>, hide: Option<DecorHide>) -> Option<Sources> {
    cols.clear();
    store.cols_under(cell.chunk().column(), cols).ok()?;
    let revision = cols.iter().fold(0u32, |r, &c| r.wrapping_mul(31).wrapping_add(object_col_revision(c)));
    let (x0, y0, side) = cell.rect();
    let inside = |x: i64, y: i64| (x0..x0 + side).contains(&(x as f64)) && (y0..y0 + side).contains(&(y as f64));
    let hide = hide.filter(|h| match *h {
        DecorHide::Placed(at) => {
            let o = FROM_L2_IDX(at.col_idx);
            inside(ivec::X(o), ivec::Y(o))
        }
        DecorHide::Scattered([x, y, _]) => cell.level == 0 && inside(x, y),
    });
    Some(Sources {
        revision,
        shown: settings_store().decor,
        hide,
    })
}

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
    let step = GROUND.scale(0).count_in(target.max(GROUND.scale(0)));
    let built = DecorBuild::build(scratch, kinds, Some(&ground), &c.placed, sources.hide, store, step, sources.shown)?;
    let index = built.bytes();
    let instances = built.instances() as u32;
    let (at, bytes) = alloc(index + instances * 4)?;
    // SAFETY: a block of the bytes the build asked for, the cell's until given back.
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
    // SAFETY: the cell's block, not given back yet; nothing reads it past this turn.
    unsafe { free((block.at as usize, block.bytes)) };
}

/// Moves what is drawn toward the frontier one whole replacement at a time:
/// a refining cell stays drawn until every wanted child is ready, coarsening
/// cells until the parent is ready. No region is ever left empty or drawn twice.
pub fn step(kinds: &[Kind], store: &Store, draw: &mut DrawList, hide: Option<DecorHide>, frame_eye: V3) {
    let c = cells();
    let mut frontier = core::mem::take(&mut c.frontier);
    solve(kinds, &mut c.stack, &mut frontier);
    xpute_core::ensure!(frontier.len() <= FRONTIER_MAX, ENOTRECOVERABLE, frontier.len());
    let top = floor::level().unwrap_or(0);
    let ancestors = move |cell: DecorCell| core::iter::successors(Some(cell), move |a| (a.level < top).then(|| a.parent())).skip(1);
    let pages = Terrain::pyramid();

    // Admission against `BLOCKS_MAX`: a coarsening comes first and is admitted
    // at the budget, since it lets more go than it takes; other new cells only
    // below it. A cell not admitted waits while what stands in stays drawn.
    c.over.clear();
    c.over.extend(c.blocks.keys().flat_map(|&k| ancestors(k)));
    c.over.sort_unstable();
    c.over.dedup();
    let mut built = 0;
    for coarsening in [true, false] {
        for &cell in &frontier {
            if built >= BUILDS_PER_FRAME {
                break;
            }
            if c.over.binary_search(&cell).is_ok() != coarsening {
                continue;
            }
            let Some(sources) = sources_of(cell, store, &mut c.cols, hide) else { continue };
            let held = c.blocks.get(&cell);
            if held.is_some_and(|b| b.sources == sources) {
                continue;
            }
            if held.is_none() && !coarsening && c.blocks.len() >= BLOCKS_MAX {
                continue;
            }
            if let Some(mut b) = build(c, cell, kinds, store, draw, sources) {
                b.drawn = c.blocks.get(&cell).is_some_and(|old| old.drawn);
                if let Some(old) = c.blocks.insert(cell, b) {
                    release(old, draw);
                }
                built += 1;
            }
        }
    }

    let wanted = |cell: &DecorCell| frontier.binary_search(cell).is_ok();
    let ready = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| blocks.contains_key(&cell) || pages.settled(cell.page());
    c.under.clear();
    for &cell in &frontier {
        let r = ready(cell, &c.blocks);
        for a in ancestors(cell) {
            *c.under.entry(a).or_insert(true) &= r;
        }
    }
    let under = &c.under;
    // Kept where wanted, or drawn last turn and still standing in for wanted
    // cells not ready.
    let kept = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| {
        wanted(&cell) || blocks[&cell].drawn && (under.get(&cell) == Some(&false) || ancestors(cell).find(wanted).is_some_and(|a| !ready(a, blocks)))
    };
    c.cells.clear();
    c.cells.extend(c.blocks.keys().copied().filter(|&k| !kept(k, &c.blocks)));
    for k in c.cells.drain(..) {
        if let Some(b) = c.blocks.remove(&k) {
            release(b, draw);
        }
    }
    c.covers.clear();
    c.covers.extend(c.blocks.keys().copied().filter(|k| under.get(k) == Some(&false) && !wanted(k)));
    c.covers.sort_unstable();
    for (&cell, b) in c.blocks.iter_mut() {
        b.drawn = !ancestors(cell).any(|a| c.covers.binary_search(&a).is_ok());
    }
    xpute_core::ensure!(c.blocks.len() <= BLOCKS_MAX, ENOTRECOVERABLE, c.blocks.len());
    c.pages.clear();
    c.pages.extend(frontier.iter().map(|c| c.page()));
    c.pages.sort_unstable();
    c.pages.dedup();
    c.frontier = frontier;
    present(c, draw, frame_eye);
}

/// Per frame, each instance as model, sprite or nothing by its projected
/// size. Within a group, models come first in the index list, then sprites;
/// the list is re-uploaded only where it changed.
fn present(c: &mut Cells, draw: &mut DrawList, frame_eye: V3) {
    let Some(eye) = eye() else { return };
    let model_px = decor_gone_px();
    let sprite_px = target_cell_px() * IMPOSTOR_GONE_CELLS;
    let sprites = &mut c.sprites;
    for (&cell, block) in c.blocks.iter().filter(|(_, b)| b.drawn) {
        let (x0, y0, _) = cell.rect();
        let translation = DrawList::translation([x0, y0, 0.0], frame_eye);
        // SAFETY: the cell's block, `bytes` long, its own to write past `index`.
        let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block.at as usize), block.bytes as usize / 4) };
        let (lanes_of, list) = words.split_at_mut(block.index as usize / 4);
        let mut extents = decor_bundle_assets().iter().flat_map(|e| e.bundle.variants.iter().map(variant_extent));
        let (mut at, mut changed) = (0usize, false);
        for (group, (offset, count)) in decor_groups(lanes_of).enumerate() {
            let extent = extents.next().unwrap_or(0.0);
            let lane = lanes(offset, count);
            let first = at;
            sprites.clear();
            for i in 0..count as usize {
                if (lanes_of[lane.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                    continue;
                }
                let f = |l: usize| f32::from_bits(lanes_of[lane.f32_at(l, i)]) as f64;
                let d = (x0 + f(0) - eye[0]).hypot(y0 + f(1) - eye[1]).hypot(f(2) - eye[2]).max(1e-3);
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

pub fn shown() -> impl Iterator<Item = (DecorCell, f64, f64, &'static [u32])> {
    cells().blocks.iter().filter(|(_, b)| b.drawn).map(|(&cell, b)| {
        let (x0, y0, _) = cell.rect();
        // SAFETY: the cell's block, `bytes` long, until given back on a later turn.
        let words: &'static [u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
        (cell, x0, y0, words)
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
    let instances = c
        .blocks
        .values()
        .map(|b| {
            // SAFETY: the cell's block, `bytes` long.
            let words: &[u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
            decor_groups(words).map(|(_, n)| n).sum::<u32>()
        })
        .sum();
    (c.blocks.len() as u32, instances)
}

/// Asked for by decor itself rather than left to what the terrain draws.
pub fn pages() -> &'static [LevelPage] {
    &cells().pages
}
