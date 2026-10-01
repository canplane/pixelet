// pxl-kernel/layer/decor/cell.rs

//! Decor as a layer of its own (issue/20261001-decor-is-its-own-layer.md):
//! its cells, its frontier, its blocks and how each instance is drawn, none
//! of it read off a terrain chunk.
//!
//! A decor cell is a column of a level's chunks, `(level, [x, y])`, and its
//! population is that level's scatter over that level's page, with the
//! placed decor of the columns under it beside it. The frontier is the cells
//! the eye wants a frame at a time: an antichain over the ground in view,
//! refined where the shared rule (`Tau::level`) asks for a finer level than a
//! cell's, and dropped where the largest instance the cell could hold is
//! smaller on the screen than the sprite's floor or out of the view. A cell
//! of the frontier is built once its page is resident — its block is
//! immutable — and given back once the frontier leaves it.
//!
//! How each instance is drawn is the presentation, a frame at a time and
//! apart from the population: the instance's own extent at its own distance,
//! its model or its sprite (`present`). A change of it rebuilds nothing.

use std::collections::HashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource};
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::lod::fault_radius;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{chunk_span_of, LevelPage, CHUNK_CELLS, GROUND};
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;

use crate::control::camera::controller::units_per_px_at;
use crate::global::Global;
use crate::layer::decor::build::{decor_gone_px, DecorBuild, Ground, Kind, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::layer::decor::scatter::SCALE_JITTER;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{object_col_revision, EntryAt, Store};
use crate::param::IMPOSTOR_GONE_CELLS;
use crate::shell::setting::store::{hidden_ground, settings_store};
use crate::world::chunk::build::{release_block, Builder};
use crate::world::observation::{eye, frustum_of_view};
use crate::world::paging::floor;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer;
use crate::world::quality;
use pxl_render::look::lod::TARGET_CELL_PX;
use pxl_spec::bake::format::Terrain;

/// A decor cell: a column of level `level`'s chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DecorCell {
    pub level: u32,
    pub at: [i64; 2],
}

impl DecorCell {
    /// The chunk at no height over it: its page and its place in the page.
    fn chunk(self) -> Chunk {
        Chunk::at(self.level, [self.at[0], self.at[1], 0])
    }

    /// Its corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        self.chunk().rect()
    }

    fn page(self) -> LevelPage {
        self.chunk().page()
    }

    fn parent(self) -> DecorCell {
        DecorCell {
            level: self.level + 1,
            at: self.at.map(|v| v.div_euclid(2)),
        }
    }

    fn children(self) -> [DecorCell; 4] {
        let [x, y] = self.at;
        let l = self.level - 1;
        [[0, 0], [1, 0], [0, 1], [1, 1]].map(|[dx, dy]| DecorCell {
            level: l,
            at: [x * 2 + dx, y * 2 + dy],
        })
    }
}

/// A cell's block: where it lies, its size, the renderer's handle, and what
/// it was made of.
struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    /// The byte the index list starts at, past the instances: a word an
    /// instance, rewritten a frame at a time by `present`.
    index: u32,
    sources: Sources,
    /// Whether the cell is drawn: a block is held hidden while it waits on
    /// the rest of a replacement (`step`).
    drawn: bool,
}

/// What a cell's population is made of beyond its page and the seed: the
/// placed decor of its columns, as they are written, the setting, and the
/// entry the hover leaves out.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    revision: u32,
    shown: bool,
    hide: Option<EntryAt>,
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

fn cells() -> &'static mut Cells {
    CELLS.get(Cells::default)
}

/// Blocks a frame builds at most: a cell's scatter walks its cells and a
/// frame has other work; what is not built this frame is the next's.
const BUILDS_PER_FRAME: u32 = 4;

/// The largest an instance of any kind could stand, in units: the largest
/// kind's extent at the largest scale it is scattered or placed at — what a
/// cell is admitted by before its block says what it holds.
fn largest_possible(kinds: &[Kind]) -> f64 {
    let extent = kinds.iter().map(|k| k.extent).fold(0.0, f64::max);
    extent * (1.0f64 + SCALE_JITTER).max(DECOR_SIZE)
}

/// The nearest a point of a cell's square comes to `eye`, the eye's height
/// over the `ground` under it counted.
fn nearest(cell: DecorCell, eye: V3, ground: f64) -> f64 {
    let (x0, y0, side) = cell.rect();
    let dx = (x0 - eye[0]).max(eye[0] - (x0 + side)).max(0.0);
    let dy = (y0 - eye[1]).max(eye[1] - (y0 + side)).max(0.0);
    dx.hypot(dy).hypot(eye[2] - ground)
}

/// The frontier for the eye: from the source's top level, a cell refined
/// where the rule wants a finer level than its own at its nearest point,
/// kept where it is seen and where its largest possible instance covers the
/// sprite's floor; none of a cell's ancestors with it.
fn solve(kinds: &[Kind], stack: &mut Vec<DecorCell>, out: &mut Vec<DecorCell>) {
    out.clear();
    stack.clear();
    let (Some(eye), Some(top)) = (eye(), floor::level()) else { return };
    let tau = quality::effective().tau();
    let frustum = frustum_of_view();
    let largest = largest_possible(kinds);
    let floor_px = *pxl_render::look::lod::TARGET_CELL_PX * crate::param::IMPOSTOR_GONE_CELLS;
    let reach = fault_radius(top);
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
        if largest / units_per_px_at(d) < floor_px {
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

/// What cell `cell`'s population is made of now.
fn sources_of(cell: DecorCell, store: &Store, cols: &mut Vec<IVec>, hide: Option<EntryAt>) -> Sources {
    cols.clear();
    store.cols_under(cell.chunk().column(), cols);
    let revision = cols.iter().fold(0u32, |r, &c| r.wrapping_mul(31).wrapping_add(object_col_revision(c)));
    let (x0, y0, side) = cell.rect();
    let hide = hide.filter(|at| {
        let o = FROM_L2_IDX(at.col_idx);
        (x0..x0 + side).contains(&(ivec::X(o) as f64)) && (y0..y0 + side).contains(&(ivec::Y(o) as f64))
    });
    Sources {
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
        c.sample.read(&around, [tx0 as i64, ty0 as i64], hidden_ground(settings_store().structures));
    }
    c.placed.clear();
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
        true => store.fold_at(target, [cx, cy], [cx + span, cy + span], sources.hide, &mut c.folded),
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
    let scratch = c.scratch.get_or_insert_with(DecorScratch::laid);
    // A cell is a whole column: every instance over it, at any height.
    let step = GROUND.scale(0).count_in(target.max(GROUND.scale(0)));
    let built = DecorBuild::build(scratch, kinds, Some(&ground), &c.placed, sources.hide, store, step, sources.shown, (f64::MIN, f64::MAX));
    let index = built.bytes();
    let instances = built.instances() as u32;
    let (at, bytes) = alloc(index + instances * 4)?;
    // SAFETY: a block of the bytes the build asked for, the cell's until it
    // is given back.
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
    release_block(block.at as usize, block.bytes);
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
/// region with nothing drawn on it, and no region is drawn twice: what
/// waits under a cell that is still drawn is held hidden.
pub fn step(kinds: &[Kind], store: &Store, draw: &mut DrawList, hide: Option<EntryAt>, frame_eye: V3) {
    let c = cells();
    let mut frontier = core::mem::take(&mut c.frontier);
    solve(kinds, &mut c.stack, &mut frontier);
    let top = floor::level().unwrap_or(0);
    let pages = Terrain::pyramid();

    let mut built = 0;
    let mut cols = core::mem::take(&mut c.cols);
    for &cell in &frontier {
        let sources = sources_of(cell, store, &mut cols, hide);
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
        cols = core::mem::take(&mut c.cols);
    }
    c.cols = cols;

    let wanted = |cell: &DecorCell| frontier.binary_search(cell).is_ok();
    let ready = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| blocks.contains_key(&cell) || pages.settled(cell.page());
    let ancestors = move |cell: DecorCell| core::iter::successors(Some(cell), move |a| (a.level < top).then(|| a.parent())).skip(1);
    // Over every wanted cell, whether all the wanted cells under it are
    // ready: a held cell there stays drawn until they are.
    c.under.clear();
    for &cell in &frontier {
        let r = ready(cell, &c.blocks);
        for a in ancestors(cell) {
            *c.under.entry(a).or_insert(true) &= r;
        }
    }
    let under = &c.under;
    let kept = |cell: DecorCell, blocks: &HashMap<DecorCell, Block>| {
        wanted(&cell) || under.get(&cell) == Some(&false) || ancestors(cell).find(wanted).is_some_and(|a| !ready(a, blocks))
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
    c.frontier = frontier;
    present(c, kinds, draw, frame_eye);
}

/// Each cell's instances listed to draw, each as its model, its sprite or
/// not at all by how large it is where it stands: the presentation, a frame
/// at a time, of a block that does not change. A group's slice of the
/// index list holds the instances drawn as models first and those drawn as
/// sprites after them; the list goes to the device again only where it
/// changed.
fn present(c: &mut Cells, kinds: &[Kind], draw: &mut DrawList, frame_eye: V3) {
    let Some(eye) = eye() else { return };
    let model_px = decor_gone_px();
    let sprite_px = *TARGET_CELL_PX * IMPOSTOR_GONE_CELLS;
    let sprites = &mut c.sprites;
    for (&cell, block) in c.blocks.iter().filter(|(_, b)| b.drawn) {
        let (x0, y0, _) = cell.rect();
        let translation = DrawList::translation([x0, y0, 0.0], frame_eye);
        // SAFETY: the block the cell holds, `bytes` long, and its own to
        // write past `index`.
        let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block.at as usize), block.bytes as usize / 4) };
        let (lanes_of, list) = words.split_at_mut(block.index as usize / 4);
        let mut extents = kinds.iter().flat_map(|k| (0..k.variant_count).map(move |_| k.extent));
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

/// Every cell shown, its corner in units and its block's words: what a hit
/// walks.
pub fn shown() -> impl Iterator<Item = (DecorCell, f64, f64, &'static [u32])> {
    cells().blocks.iter().filter(|(_, b)| b.drawn).map(|(&cell, b)| {
        let (x0, y0, _) = cell.rect();
        // SAFETY: the block the cell holds, `bytes` long, until it is given
        // back on a later turn.
        let words: &'static [u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
        (cell, x0, y0, words)
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

/// Instances the build's lanes had no room for, since the kernel started.
pub fn dropped() -> u32 {
    cells().scratch.as_ref().map_or(0, |s| s.dropped)
}

/// The cells holding a block, and the instances their blocks hold.
pub fn counts() -> (u32, u32) {
    let c = cells();
    let instances = c
        .blocks
        .values()
        .map(|b| {
            // SAFETY: the block the cell holds, `bytes` long.
            let words: &[u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
            decor_groups(words).map(|(_, n)| n).sum::<u32>()
        })
        .sum();
    (c.blocks.len() as u32, instances)
}

/// The terrain pages the frontier's cells scatter over, each once: what
/// decor reads of the ground, asked for by decor (world/pyramid.rs
/// `step`'s readers) rather than left to what the terrain draws.
pub fn pages() -> &'static [LevelPage] {
    &cells().pages
}
