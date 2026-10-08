// pxl-kernel/chunk/decor/cell.rs

//! Decor as a payload of the chunks the terrain materializes: each mounted
//! chunk holds the decor standing inside it, built over its zoom's tile and
//! the object store, and drawn while the chunk is. A block is immutable once
//! built; how each instance is drawn (`present`) is decided per frame and
//! rebuilds nothing.

use crate::layer::Query;
use std::collections::HashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_IMPOSTOR};
use pxl_spec::space::camera::grid::CHUNK_CELLS;
use pxl_spec::space::coord::v3::{len3, sub3, V3};
use pxl_spec::space::coord::world::{nearest, nearest_box};
use pxl_spec::space::coord::zoom::cube;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_guest::sched::frame_budget::FrameBudget;

use crate::chunk::build::draws_as_held;
use crate::chunk::decor::build::{decor_gone_px, decor_sprite_px, DecorBuild, DecorHide, Ground};
use crate::chunk::decor::instance::{decor_groups, DecorScratch, GroupLanes};
use crate::chunk::mesh_slot::OCCUPANCY;
use crate::chunk::source::{objects_met, Met};
use crate::chunk::table::Scheduler;
use crate::chunk::Chunk;
use crate::kernel;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::Store;
use crate::layer::terrain::scatter::SCALE_JITTER;
use crate::layer::Layer;
use crate::setting::store::settings_store;
use crate::view::frame::pose::units_per_px_at;
use crate::view::observation::{eye, observation};
use pxl_spec::bake::format::TerrainTile;
use pxl_spec::cell::decor::{DecorKind, Variant, KINDS};
use pxl_spec::space::coord::zoom::Zoom;
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
    /// Its chunk is drawn and in the frustum.
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
    blocks: HashMap<Chunk, Block>,
    scratch: Option<DecorScratch>,
    folded: HashMap<[i64; 3], u32>,
    /// Kept so a settled turn asks the heap for nothing.
    mounted: Vec<(Chunk, bool, bool)>,
    gone: Vec<Chunk>,
    sprites: Vec<u32>,
}

static CELLS: Global<Cells> = Global::new();

fn cells() -> &'static mut Cells {
    CELLS.get(Cells::default)
}

/// One a mounted chunk.
pub const BLOCKS_MAX: usize = OCCUPANCY;

/// What a chunk is admitted by before its block says what it holds.
fn largest_possible(kinds: &[DecorKind]) -> f64 {
    let extent = kinds.iter().map(|k| k.extent()).fold(0.0, f64::max);
    extent * (1.0f64 + SCALE_JITTER).max(DECOR_SIZE)
}

/// Whether anything a chunk could hold is in reach and over the sprite's floor.
fn may_show(chunk: Chunk, largest: f64) -> bool {
    let Some(o) = observation() else { return false };
    let near = nearest_box(&chunk.region(), o.eye[0]).nearest(o.eye);
    let d = len3(&sub3(&near, &o.eye)).max(1e-3);
    d <= o.far() && largest / units_per_px_at(d) >= decor_sprite_px()
}

fn sources_of(chunk: Chunk, hide: Option<DecorHide>) -> Option<Sources> {
    let (x0, y0, side) = chunk.rect();
    let inside = |x: f64, y: f64| (x0..x0 + side).contains(&x) && (y0..y0 + side).contains(&y);
    // Only where the chunk draws it as it is held: a fold is no one's to hide.
    let own = draws_as_held(chunk, GROUND.cell());
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

fn build(c: &mut Cells, chunk: Chunk, kinds: &[DecorKind], store: &Store, draw: &mut DrawList, sources: Sources) -> Option<Block> {
    let terrain = TerrainTile::residency();
    // The tile the terrain draws the chunk from, read over the chunk whose
    // side is that many of its cells; past the leaves, the chunk over it as wide.
    let src = terrain.read(chunk.node(), chunk.cell_zoom()).value()?.from;
    let mut over = chunk;
    while GROUND.read_side(over.zoom(), over.cell_zoom()) < CHUNK_CELLS || GROUND.read_zoom(over.cell_zoom()) > src.zoom {
        over = over.parent()?;
    }
    let (zoom, side) = (GROUND.read_zoom(over.cell_zoom()), GROUND.read_side(over.zoom(), over.cell_zoom()));
    let (x0, y0, _) = over.rect();
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (tx0, ty0) = GROUND.read_at(over.column(), over.cell_zoom());
    // Read over `over`, whose tile is `src` itself.
    let own_view = terrain.read(over.node(), over.cell_zoom()).value()?.view();
    // Interaction reads what is drawn as it is held, never a fold of it.
    let own = draws_as_held(chunk, GROUND.cell());
    let target = GROUND.cell_zoom(zoom);
    match target < GROUND.cell() && sources.shown {
        true => store.read(over.node(), target).value()?.fold(0, sources.hide.and_then(DecorHide::placed), &mut c.folded).ok()?,
        false => c.folded.clear(),
    }
    let ground = Ground {
        source: own_view,
        zoom,
        x0,
        y0,
        tx0,
        ty0,
        side,
        folded: (target < GROUND.cell()).then_some(&c.folded),
    };
    let scratch = c.scratch.get_or_insert_with(DecorScratch::default);
    let built = DecorBuild::build(scratch, kinds, Some(&ground), sources.hide.filter(|_| own), store, sources.shown, chunk)?;
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

/// Builds the decor of the chunks the terrain has mounted, within what is
/// left of the turn as the chunks' builds are, and lets go of what an
/// unmounted chunk held.
pub fn step(kinds: &[DecorKind], store: &Store, sched: &Scheduler, draw: &mut DrawList, hide: Option<DecorHide>, frame_eye: V3, budget: &FrameBudget) {
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
            let Some(b) = build(c, chunk, kinds, store, draw, sources) else { return false };
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
        let translation = DrawList::translation([nearest(x0, frame_eye[0]), y0, 0.0], frame_eye);
        let x0 = nearest(x0, eye[0]);
        let [zlo, zhi] = block.heights.map(f64::from);
        let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
        let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
        let nearest = len3(&[gap(x0, x0 + side, eye[0]), gap(y0, y0 + side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
        let farthest = len3(&[reach(x0, x0 + side, eye[0]), reach(y0, y0 + side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
        // SAFETY: the chunk's block, `bytes` long, its own to write past `index`.
        let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(block.at as usize), block.bytes as usize / 4) };
        let (lanes_of, list) = words.split_at_mut(block.index as usize / 4);
        let mut extents = KINDS.iter().flat_map(|k| k.variants.iter().map(Variant::extent));
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
