// pxl-kernel/scene/decor/cell.rs

//! Decor in the eye's cubes (view/observation.rs `cubes`): each holds the
//! decor standing inside it, built over its zoom's tile and the object
//! store, and drawn while it is in view. A block is immutable once built; how
//! each instance is drawn (`present`) is decided per frame and rebuilds
//! nothing.

use crate::layer::Query;
use fnv::FnvHashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR};
use pxl_spec::space::camera::grid::CHUNK_CELLS;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::{nearest, nearest_box};
use pxl_spec::space::coord::zoom::{cube, units};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_guest::sched::frame_budget::FrameBudget;

use crate::chunk::Chunk;
use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::Base;
use crate::scene::decor::build::{decor_gone_px, decor_sprite_px, DecorBuild, DecorHide, Ground};
use crate::scene::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::scene::frame::Frame;
use crate::setting::store::settings_store;
use crate::view::frame::pose::units_per_px_at;
use crate::view::observation::{eye, observation};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use xpute_guest::global::Global;

struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    /// Byte offset of the index list past the instances, rewritten each frame by `present`.
    index: u32,
    /// The lowest and highest instance, in units.
    heights: [f32; 2],
    sources: Sources,
    /// Its cube is in view.
    drawn: bool,
}

impl Block {
    fn words(&self) -> &'static [u32] {
        // SAFETY: the chunk's block, `bytes` long, until given back on a later turn.
        unsafe { core::slice::from_raw_parts(ptr_at::<u32>(self.at as usize), self.bytes as usize / 4) }
    }
}

/// What a population depends on beyond its tile and the seed.
#[derive(Clone, PartialEq, Eq)]
struct Sources {
    /// The zoom of the tile it is built from, coarser until its own comes.
    read_from: Zoom,
    objects: Option<Vec<Met>>,
    shown: bool,
    hide: Option<DecorHide>,
}

#[derive(Default)]
struct Cells {
    blocks: FnvHashMap<Chunk, Block>,
    scratch: Option<DecorScratch>,
    /// Kept so a settled turn asks the heap for nothing.
    mounted: Vec<(Chunk, bool, bool)>,
    /// The same, in key order, for what is held.
    keyed: Vec<Chunk>,
    gone: Vec<Chunk>,
    sprites: Vec<u32>,
}

static CELLS: Global<Cells> = Global::new();

fn cells() -> &'static mut Cells {
    CELLS.get(Cells::default)
}

/// One a cube in view, the nearest first: a cube spans cells of the camera
/// grid (view/observation.rs `cubes`), so no more than its cells. A block let
/// go is freed before any is built.
pub const BLOCKS_MAX: usize = crate::param::GRID_CELLS_MAX as usize;

/// What a chunk is admitted by before its block says what it holds.
fn largest_possible(decors: &[Decor]) -> f64 {
    decors.iter().map(Decor::extent).fold(0.0, f64::max)
}

/// Whether anything a chunk could hold is in reach and over the sprite's floor.
fn may_show(chunk: Chunk, largest: f64) -> bool {
    let Some(o) = observation() else { return false };
    let near = nearest_box(&chunk.region(), o.eye[0]).nearest(o.eye);
    let d = v3::len(&v3::sub(&near, &o.eye)).max(1e-3);
    d <= o.far() && largest / units_per_px_at(d) >= decor_sprite_px()
}

fn sources_of(chunk: Chunk, hide: Option<DecorHide>) -> Option<Sources> {
    let (x0, y0, side) = chunk.rect();
    let inside = |x: f64, y: f64| (x0..x0 + side).contains(&x) && (y0..y0 + side).contains(&y);
    // Only where the chunk draws it as it is held: a fold is no one's to hide.
    let own = chunk.cell_zoom() >= GROUND.cell();
    let hide = hide.filter(|h| {
        own && match *h {
            DecorHide::Placed(e) => {
                let o = nearest_box(&cube(e.zoom(), e.at()), x0 + side / 2.0);
                inside((o.min[0] + o.max[0]) / 2.0, (o.min[1] + o.max[1]) / 2.0)
            }
            DecorHide::Scattered([x, y, _]) => inside(x as f64, y as f64),
        }
    });
    Some(Sources {
        read_from: TerrainTile::residency().read(chunk.node(), chunk.cell_zoom()).value()?.from.zoom,
        objects: objects_met(&kernel().objects, chunk),
        shown: settings_store().decor,
        hide,
    })
}

fn build(c: &mut Cells, chunk: Chunk, decors: &[Decor], store: &Store, draw: &mut DrawList, sources: Sources) -> Option<Block> {
    let terrain = TerrainTile::residency();
    // The tile the terrain draws the chunk from, read over the chunk whose
    // side is that many of its cells; past the leaves, the chunk over it as wide.
    let src = terrain.read(chunk.node(), chunk.cell_zoom()).value()?.from;
    let mut over = chunk;
    while GROUND.read_side(over.zoom(), over.cell_zoom()) < CHUNK_CELLS || GROUND.read_zoom(over.cell_zoom()) > src.zoom {
        over = over.parent()?;
    }
    let (x0, y0, _) = over.rect();
    // Interaction reads what is drawn as it is held, never a fold of it.
    let own = chunk.cell_zoom() >= GROUND.cell();
    let ground = Ground {
        node: over.column(),
        cell: over.cell_zoom(),
        origin: [x0 as i64, y0 as i64],
    };
    let scratch = c.scratch.get_or_insert_with(DecorScratch::default);
    let built = DecorBuild::build(scratch, decors, Some(&ground), sources.hide.filter(|_| own), store, sources.shown, chunk)?;
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

/// Builds the decor of the eye's cubes, within what is left of the turn, and
/// lets go of what a cube no longer the eye's held.
pub fn step(decors: &[Decor], store: &Store, frame: &Frame, draw: &mut DrawList, hide: Option<DecorHide>, budget: &FrameBudget) {
    let c = cells();
    let mut mounted = core::mem::take(&mut c.mounted);
    mounted.clear();
    // The eye's cubes in view, nearest first: what is built first is what is seen nearest.
    if let Some(o) = observation() {
        crate::view::observation::cubes(&o, crate::view::lens::tau(), &mut |(zoom, at)| {
            let chunk = Chunk::indexed(zoom, at);
            let b = chunk.region();
            let center = core::array::from_fn(|k| (b.min[k] + b.max[k]) / 2.0);
            if frame.sees(center, units(zoom) * 3f64.sqrt() / 2.0) {
                mounted.push((chunk, true, true));
            }
        });
        let near = |c: &Chunk| v3::len(&v3::sub(&nearest_box(&c.region(), o.eye[0]).nearest(o.eye), &o.eye));
        mounted.sort_unstable_by(|a, b| near(&a.0).total_cmp(&near(&b.0)));
        mounted.truncate(BLOCKS_MAX);
    }
    let mut keyed = core::mem::take(&mut c.keyed);
    keyed.clear();
    keyed.extend(mounted.iter().map(|m| m.0));
    keyed.sort_unstable();
    let frame_eye = frame.eye;
    let largest = largest_possible(decors);
    let held = |k: &Chunk| keyed.binary_search(k).is_ok() && may_show(*k, largest);
    c.gone.clear();
    c.gone.extend(c.blocks.keys().copied().filter(|k| !held(k)));
    for k in c.gone.drain(..) {
        if let Some(b) = c.blocks.remove(&k) {
            release(b, draw);
        }
    }
    budget.run(
        mounted.iter().map(|m| m.0),
        |chunk| {
            if !may_show(chunk, largest) {
                return false;
            }
            let Some(sources) = sources_of(chunk, hide) else { return false };
            if c.blocks.get(&chunk).is_some_and(|b| b.sources == sources) {
                return false;
            }
            let Some(b) = build(c, chunk, decors, store, draw, sources) else { return false };
            if let Some(old) = c.blocks.insert(chunk, b) {
                release(old, draw);
            }
            true
        },
        None,
    );
    for &(chunk, drawn, seen) in &mounted {
        if let Some(b) = c.blocks.get_mut(&chunk) {
            b.drawn = drawn && seen;
        }
    }
    c.mounted = mounted;
    c.keyed = keyed;
    present(c, draw, frame_eye);
}

/// Per frame, each instance as model, sprite or nothing by its projected
/// size; a group surely one or the other over the chunk's nearest and farthest
/// distance is settled whole. Models come first in a group's index list, then
/// sprites; the list is re-uploaded only where it changed.
fn present(c: &mut Cells, draw: &mut DrawList, frame_eye: V3) {
    let Some(eye) = eye() else { return };
    let sprites = &mut c.sprites;
    for (&chunk, block) in c.blocks.iter().filter(|(_, b)| b.drawn) {
        let (x0, y0, side) = chunk.rect();
        let translation = DrawList::translation([nearest(x0, frame_eye[0]), y0, 0.0], frame_eye);
        let shown = Shown {
            handle: block.handle,
            at: block.at,
            index: block.index,
            bytes: block.bytes,
            corner: [nearest(x0, eye[0]), y0],
            side,
            heights: block.heights,
        };
        // The ground grows decor at its own scale.
        let extents = DECORS.iter().flat_map(|k| k.variants.iter().map(|v| (v.extent(), [1.0, 1.0])));
        present_block(draw, &shown, translation, eye, (0..).zip(extents), 1.0, sprites);
    }
}

/// A block of decor lanes as `present_block` reads it: its square's corner
/// about the eye's lap, its side, and the heights its instances stand between.
pub struct Shown {
    pub handle: u32,
    pub at: u32,
    pub index: u32,
    pub bytes: u32,
    pub corner: [f64; 2],
    pub side: f64,
    pub heights: [f32; 2],
}

/// Lists one block's instances, model, sprite or nothing each, under the
/// draw groups `groups` names in the block's order, each with its model's
/// extent and the least and most scale its instances are drawn at. Under an
/// `opacity` of one the models are laid over the world blended, and a sprite,
/// which cannot be, is left out.
#[allow(clippy::too_many_arguments)]
pub fn present_block(draw: &mut DrawList, b: &Shown, translation: [f32; 3], eye: V3, groups: impl Iterator<Item = (u32, (f64, [f64; 2]))>, opacity: f32, sprites: &mut Vec<u32>) {
    let (model_px, sprite_px) = (decor_gone_px(), decor_sprite_px());
    let [x0, y0] = b.corner;
    let [zlo, zhi] = b.heights.map(f64::from);
    let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
    let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
    let nearest = v3::len(&[gap(x0, x0 + b.side, eye[0]), gap(y0, y0 + b.side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
    let farthest = v3::len(&[reach(x0, x0 + b.side, eye[0]), reach(y0, y0 + b.side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
    // SAFETY: the block, `bytes` long, its owner's to write past `index`.
    let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
    let (lanes_of, list) = words.split_at_mut(b.index as usize / 4);
    let (mut at, mut changed) = (0usize, false);
    for ((offset, count), (group, (extent, [least, most]))) in decor_groups(lanes_of).zip(groups) {
        let lane = lanes(offset, count);
        let first = at;
        sprites.clear();
        let gone = extent * most / units_per_px_at(nearest) < sprite_px;
        let modeled = extent * least / units_per_px_at(farthest) >= model_px;
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
            let d = v3::len(&[x0 + f(0) - eye[0], y0 + f(1) - eye[1], f(2) - eye[2]]).max(1e-3);
            let px = extent * f(4) / units_per_px_at(d);
            if px >= model_px {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
            } else if px >= sprite_px && opacity >= 1.0 {
                sprites.push(i as u32);
            }
        }
        let models = (at - first) as u32;
        changed |= list[at..at + sprites.len()] != sprites[..];
        list[at..at + sprites.len()].copy_from_slice(sprites);
        at += sprites.len();
        let index = b.index / 4 + first as u32;
        if models > 0 {
            let pipeline = if opacity < 1.0 { PIPELINE_GHOST_DECOR } else { PIPELINE_DECOR };
            draw.draw_instances(pipeline, b.handle, group, models, [offset, count, index], translation, opacity);
        }
        if !sprites.is_empty() {
            draw.draw_instances(PIPELINE_IMPOSTOR, b.handle, group, sprites.len() as u32, [offset, count, index + models], translation, 1.0);
        }
    }
    if changed {
        draw.rewrite_part(b.handle, b.at, b.index, at as u32 * 4);
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

/// One leaf a chunk's decor stands among: its zoom, catalog number and place
/// from the chunk's corner at its zoom.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Met {
    zoom: Zoom,
    value: u16,
    at: [i32; 3],
}

/// Every leaf meeting the chunk, air included, sorted; none where the heap refused it.
pub fn objects_met(store: &Store, k: Chunk) -> Option<Vec<Met>> {
    let (zoom, corner) = k.node();
    let mut out: Vec<Met> = Vec::new();
    let mut refused = false;
    store.read(k.node(), k.cell_zoom()).value()?.entries(&mut |e| {
        let from = if e.zoom >= zoom {
            first_descendant_of(corner, e.zoom - zoom)
        } else {
            ancestor_of(corner, zoom - e.zoom)
        };
        refused |= out.try_reserve(1).is_err();
        if !refused {
            out.push(Met {
                zoom: e.zoom,
                value: e.state as u16,
                at: core::array::from_fn(|a| (e.at[a] - from[a]) as i32),
            });
        }
    });
    (!refused).then(|| {
        out.sort_unstable();
        out
    })
}
