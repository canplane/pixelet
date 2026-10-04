// pxl-kernel/layer/decor/cell.rs

//! Decor as its own layer. A decor cell is a column of a level's chunks; the
//! frontier is the cells the eye wants, refined by `Tau::level` and dropped
//! where the largest possible instance is below the sprite's floor or out of
//! view. A block is immutable once built; how each instance is drawn
//! (`present`) is decided per frame and rebuilds nothing.

use std::collections::HashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource};
use pxl_spec::procedural::Frame;
use pxl_spec::space::hierarchy::{child_of, parent_of};
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::lod::fault_radius;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND};
use pxl_spec::space::v3::{len3, sub3, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::sched::frame_budget::FrameBudget;

use crate::control::camera::controller::units_per_px_at;
use crate::layer::decor::build::{decor_gone_px, decor_sprite_px, variant_extent, DecorBuild, DecorHide, Ground, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{object_col_revision, Store};
use crate::param::IMPOSTOR_GONE_CELLS;
use crate::shell::setting::store::{hidden_ground, settings_store};
use crate::world::chunk::build::Builder;
use crate::world::observation::{eye, observation};
use crate::world::paging::floor;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use crate::world::quality;
use pxl_spec::bake::format::Terrain;
use xpute_runtime::global::Global;
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
    /// The terrain pages the frontier scatters over, each once, in order.
    pages: Vec<LevelPage>,
    blocks: HashMap<DecorCell, Block>,
    scratch: Option<DecorScratch>,
    sample: ChunkSample,
    cols: Vec<IVec>,
    placed: Vec<Placed>,
    folded: HashMap<[i64; 3], u32>,
    /// What a turn works in, kept so a settled turn asks the heap for
    /// nothing: the solve's stack, the replacement's state over each wanted
    /// cell's ancestors, the cells it gives back or that cover others, and a
    /// group's sprites.
    stack: Vec<DecorCell>,
    under: HashMap<DecorCell, bool>,
    cells: Vec<DecorCell>,
    covers: Vec<DecorCell>,
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

/// The most cells a frontier holds, whatever the eye: a sum over the levels
/// of how many cells the solve can reach at each (`solve`), the frustum and
/// the sprite's floor, which only drop cells, left out.
///
/// At the source's top the solve starts from the square of cells within
/// `fault_radius(top)` of the eye's cell, one more each way: `2n + 1` a side
/// with `n = ⌈r / side⌉ + 1`. Under it a cell is a child of one refined,
/// which is refined only where the rule wants a finer level at its nearest
/// point — nearer than `split_distance(L + 1) / τ`, no further than
/// `fault_radius(L)` since τ ≥ 1, measured in 3D and so no nearer than over
/// the ground. The parents of level L's cells are squares of level L + 1
/// meeting a disc of that radius: an interval of `2r` meets at most
/// `⌊2r / side⌋ + 2` of them a side, and each has two children a side. A
/// split distance and a chunk grow by the same factor a level, so the count
/// is one number for every level under the top, and the top's is another;
/// the bound is the top's and `TOP_LEVEL` of the other, the most levels a
/// source has under its top.
pub const FRONTIER_MAX: usize = {
    let side = chunk_span_of(TOP_LEVEL) as u64;
    let n = (fault_radius(TOP_LEVEL) as u64).div_ceil(side) + 1;
    let top = (2 * n + 1) * (2 * n + 1);
    let across = 2 * ((2 * fault_radius(0) as u64) / chunk_span_of(1) as u64 + 2);
    (top + TOP_LEVEL as u64 * across * across) as usize
};

/// Blocks decor holds at once at most: the cells of the frontier wanted now,
/// and those drawn the turn before that still stand in for some not ready
/// (`step`) — the second no more than a frontier while the cover drawn is
/// one frontier's. A wanted parent left unready while the eye moves on can
/// keep an older cover under it beside a newer one elsewhere, and this
/// bound is then not one the replacement rule proves.
pub const BLOCKS_MAX: usize = 2 * FRONTIER_MAX;

/// Blocks a frame builds at most: a cell's scatter walks its cells and a
/// frame has other work; what is not built this frame is the next's.
const BUILDS_PER_FRAME: u32 = 4;

/// What a cell is admitted by before its block says what it holds.
fn largest_possible(kinds: &[Kind]) -> f64 {
    let extent = kinds.iter().map(|k| k.extent).fold(0.0, f64::max);
    extent * (1.0f64 + SCALE_JITTER).max(DECOR_SIZE)
}

/// Whether anything a chunk could hold is in reach and over the sprite's floor.
fn may_show(chunk: Chunk, largest: f64) -> bool {
    let Some(o) = observation() else { return false };
    let region = chunk.region();
    let near = region.nearest(o.eye);
    let d = len3(&sub3(&near, &o.eye)).max(1e-3);
    d <= o.far && largest / units_per_px_at(d) >= decor_sprite_px()
}

/// The frontier for the eye: from the source's top level, a cell refined
/// where the rule wants a finer level than its own at its nearest point,
/// kept where it is seen and where its largest possible instance covers the
/// sprite's floor; none of a cell's ancestors with it.
fn solve(kinds: &[Kind], stack: &mut Vec<DecorCell>, out: &mut Vec<DecorCell>) {
    out.clear();
    stack.clear();
    // The view the pyramids observe: its eye, reach, frustum and ground.
    let Some(Observation {
        eye,
        frustum,
        far: reach,
        top,
        ground,
        ..
    }) = observation()
    else {
        return;
    };
    let tau = quality::effective().tau();
    let largest = largest_possible(kinds);
    let floor_px = target_cell_px() * crate::param::IMPOSTOR_GONE_CELLS;
    let side = chunk_span_of(top);
    let (cx, cy) = ((eye[0] / side).floor() as i64, (eye[1] / side).floor() as i64);
    let n = (reach / side).ceil() as i64 + 1;
    for y in cy - n..=cy + n {
        for x in cx - n..=cx + n {
            stack.push(DecorCell { level: top, at: [x, y] });
        }
    }
    while let Some(cell) = stack.pop() {
        let d = nearest(cell, eye, ground).max(1e-3);
        if d > reach || largest / units_per_px_at(d) < floor_px {
            continue;
        }
        let (x0, y0, s) = cell.rect();
        if !frustum.is_none_or(|f| f.sees([x0 + s / 2.0, y0 + s / 2.0, ground], s * 0.75 + largest)) {
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

/// What cell `cell`'s population is made of now; none where the heap
/// refused the room to list its columns, and the cell stays as it is.
fn sources_of(cell: DecorCell, store: &Store, cols: &mut Vec<IVec>, hide: Option<EntryAt>) -> Option<Sources> {
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
    let page = pages.paging().get(cell.page())?;
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
        true => store.fold_at(target, [cx, cy], [cx + span, cy + span], sources.hide, &mut c.folded).ok()?,
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
    // SAFETY: the block the cell took and has not given back; nothing reads
    // it past this turn.
    unsafe { free((block.at as usize, block.bytes)) };
}

/// Moves decor toward the eye's frontier, a frame at a time.
///
/// The frontier is what is wanted; what is drawn is a cover of the ground
/// in view that reaches it one whole replacement at a time, as the
/// terrain's does. A cell refining into its children stays drawn until
/// every wanted cell under it is ready, and they are drawn in the frame it
/// goes; cells coarsening into their parent stay drawn until the parent
/// is ready. Ready is a built block, or a page a coarser one settles,
/// which has nothing to scatter. So a change of level never leaves a
/// region with nothing drawn on it, and no region is drawn twice: a wanted
/// cell waiting under one still drawn is held hidden, and a cell neither
/// wanted nor drawn the turn before is let go.
pub fn step(kinds: &[Kind], store: &Store, draw: &mut DrawList, hide: Option<EntryAt>, frame_eye: V3) {
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
    let mut cols = core::mem::take(&mut c.cols);
    for &cell in &frontier {
        let Some(sources) = sources_of(cell, store, &mut cols, hide) else { continue };
        if c.blocks.get(&cell).is_some_and(|b| b.sources == sources) {
            continue;
        }
        if built >= BUILDS_PER_FRAME {
            continue;
        }
        c.cols = core::mem::take(&mut cols);
        if let Some(mut b) = build(c, cell, kinds, store, draw, sources) {
            b.drawn = c.blocks.get(&cell).is_some_and(|old| old.drawn);
            if let Some(old) = c.blocks.insert(cell, b) {
                release(old, draw);
            }
            built += 1;
        }
    }

    let wanted = |cell: &DecorCell| frontier.binary_search(cell).is_ok();
    let ready = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| blocks.contains_key(&cell) || pages.paging().settled(cell.page());
    c.under.clear();
    for &cell in &frontier {
        let r = ready(cell, &c.blocks);
        for a in ancestors(cell) {
            *c.under.entry(a).or_insert(true) &= r;
        }
    }
    let under = &c.under;
    // A cell is kept where it is wanted, or where it was drawn the turn
    // before and still stands in for wanted cells not ready: as a cover over
    // them, or under a wanted parent not ready. Nothing else is held — a
    // cell built for a frontier the eye has since left, and never drawn, is
    // let go rather than kept as history.
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
    // Asked for by decor itself rather than left to what the terrain draws.
    Terrain::pyramid().require::<DecorCell>(&c.pages);
    c.frontier = frontier;
    present(c, draw, frame_eye);
}

/// Each cell's instances listed to draw, each as its model, its sprite or
/// not at all by how large it is where it stands: the presentation, a frame
/// at a time, of a block that does not change. A group's slice of the
/// index list holds the instances drawn as models first and those drawn as
/// sprites after them; the list goes to the device again only where it
/// changed.
fn present(c: &mut Cells, draw: &mut DrawList, frame_eye: V3) {
    let Some(eye) = eye() else { return };
    let model_px = decor_gone_px();
    let sprite_px = *TARGET_CELL_PX * IMPOSTOR_GONE_CELLS;
    let sprites = &mut c.sprites;
    for (&cell, block) in c.blocks.iter().filter(|(_, b)| b.drawn) {
        let (x0, y0, _) = cell.rect();
        let translation = DrawList::translation([x0, y0, 0.0], frame_eye);
        let [zlo, zhi] = block.heights.map(f64::from);
        let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
        let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
        let nearest = len3(&[gap(x0, x0 + side, eye[0]), gap(y0, y0 + side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
        let farthest = len3(&[reach(x0, x0 + side, eye[0]), reach(y0, y0 + side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
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
