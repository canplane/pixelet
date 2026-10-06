// pxl-kernel/layer/object/store.rs

//! Placed objects, kept as their one record: each leaf's catalog value at its
//! node, in one sparse tree a column (`space/hierarchy/sparse_tree.rs`). A
//! write arrives as `<key, id>` (`morton::key_of`). What a node at any zoom
//! shows is read from the leaves over and under it, folding by the objects'
//! majority, and never kept.

use std::collections::HashMap;

use fnv::FnvBuildHasher;
use std::collections::TryReserveError;

pub use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::fold::majority;
use pxl_spec::cell::object::{decor_id_from_num, decor_num_id, object_value_is_air, object_value_is_decor, object_value_is_voxel, placeable_cube_num_id, placeable_cube_of, PlaceableCube};
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::{Aabb, V3};
use pxl_spec::space::coord::world::{around, nearest, REACH, ROUND};
use pxl_spec::space::coord::zoom::{base_of, count, cube, index_of, of_side, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{key_of, node_of};
use pxl_spec::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_spec::space::hierarchy::tree::{child_of, first_descendant_of, region, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use crate::kernel;
use crate::layer::object::repository;
use crate::layer::{Answer, Bound, Query};
use crate::world::change::request_world_tick;
use crate::world::chunk::decor::build::{scatters_at, Draw};
use crate::world::chunk::source::chunk_sources_changed;
use crate::world::query::Access;
use crate::world::view::observation::objects_written;

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Solid { cube: PlaceableCube },
    Decor { decor_id: DecorId },
}

/// A leaf as an asker meets it: its cube, X in the asker's frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    pub zoom: Zoom,
    pub at: [i64; 3],
    pub state: u32,
}

impl Entry {
    pub fn id(&self) -> EntryAt {
        EntryAt::new(self.zoom, self.at)
    }

    pub fn size(&self) -> f64 {
        units(self.zoom)
    }

    pub fn corner(&self) -> Pvf3 {
        base_of(self.zoom, self.at)
    }

    pub fn cube(&self) -> Aabb {
        cube(self.zoom, self.at)
    }

    /// Not for air, which is the eraser's marker.
    pub fn kind(&self) -> EntryKind {
        if object_value_is_decor(self.state) {
            EntryKind::Decor {
                decor_id: decor_id_from_num(self.state),
            }
        } else {
            EntryKind::Solid {
                // Every value the store writes is one the catalog gives out.
                cube: placeable_cube_of(self.state).or_bug(Errno::ENOTRECOVERABLE),
            }
        }
    }
}

fn read_state(coord: &Pvf3) -> u32 {
    u32::from(kernel().objects.folded_value(OBJECT_FINEST, index_of(OBJECT_FINEST, coord)))
}

pub(super) fn apply(writes: impl IntoIterator<Item = Write>) {
    let regions = kernel().objects.write(writes);
    if !regions.is_empty() {
        written(&regions);
    }
}

/// Where the objects changed: what draws them compares what it draws at its
/// next turn, and what is worked out from them redoes what lies there.
pub(super) fn written(regions: &[Aabb]) {
    chunk_sources_changed();
    objects_written(regions);
    request_world_tick();
}

/// Whether any leaf, air included, is written in the cell `at` lies in.
pub fn holds_entry(at: &Pvf3) -> bool {
    kernel().objects.holds(OBJECT_COARSEST, index_of(OBJECT_COARSEST, at))
}

/// What stands in the cell `at` lies in, air left out.
pub fn objects_at(at: &Pvf3) -> Vec<Entry> {
    let mut out = Vec::new();
    kernel().objects.each_in(OBJECT_COARSEST, index_of(OBJECT_COARSEST, at), &mut |entry| {
        if !object_value_is_air(entry.state) {
            out.push(*entry);
        }
    });
    out
}

pub fn get_placed_object(at: EntryAt) -> Option<Entry> {
    let state = u32::from(kernel().objects.placed(at.zoom(), at.at())?);
    (!object_value_is_air(state)).then_some(Entry { zoom: at.zoom(), at: at.at(), state })
}

/// The never-written AIR entry the eraser's preview injects into the drawn
/// build where it hovers bare terrain: the cube of `size` holding `at`.
pub fn carve_entry(at: &Pvf3, size: f64) -> EntryAt {
    let zoom = of_side(size);
    EntryAt::new(zoom, index_of(zoom, at))
}

// ---- Mutations ----

/// A write of VOID collapses its path back, so a redundant record never sits in a save.
fn write_object_value(coord: &Pvf3, size: f64, val: u32) {
    submit(write_of(coord, size, val));
}

/// A write is made here at once and sent to be kept.
fn submit(write: Write) {
    apply([write]);
    repository::send(&write);
}

fn write_of(coord: &Pvf3, size: f64, val: u32) -> Write {
    // Size is a power of two by construction; the nearest zoom only guards
    // against float noise.
    let zoom = of_side(size);
    Write {
        key: key(zoom, index_of(zoom, coord)),
        id: val,
    }
}

/// Whether a whole-cell pick of a look only undoes a carve: the terrain must
/// show that look there, and the cell must already hold an AIR leaf (the look
/// is per column, so without this every cube stacked on matching ground was
/// dropped).
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube) -> bool {
    if size < units(GROUND.cell()) {
        return false;
    }
    let PlaceableCube::Ground(look) = cube else { return false };
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let Some(ground) = Access::now().at(at, LEAF_ZOOM).exact() else { return false };
    if ground.cell != look as u8 {
        return false;
    }
    object_value_is_air(read_state(at))
}

/// A redundant re-paint of a carve writes VOID, as remove() does.
pub fn place(at: &Pvf3, size: f64, cube: PlaceableCube) {
    let val = if place_value_is_redundant(at, size, cube) { VOID } else { placeable_cube_num_id(cube) };
    write_object_value(at, size, val);
}

/// Placing the decor a scatter stands in a cell an erase emptied writes
/// VOID, so the scattered instance returns as it was.
pub fn place_decor(at: &Pvf3, size: f64, decor_id: DecorId) {
    let value = if restores_scattered(at, size, decor_id).is_some() { VOID } else { decor_num_id(decor_id) };
    write_object_value(at, size, value);
}

/// The scattered instance `place_decor` would bring back, for its preview.
pub fn restores_scattered(at: &Pvf3, size: f64, decor_id: DecorId) -> Option<Draw> {
    (size >= units(GROUND.cell()) && object_value_is_air(read_state(at))).then(|| scatters_at(decor_num_id(decor_id), at))?
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: &Pvf3, size: f64) {
    write_object_value(at, size, cube_types::object::air as u32);
}

pub fn remove(at: EntryAt) {
    submit(Write {
        key: key(at.zoom(), at.at()),
        id: VOID,
    });
}

pub const VOID: u32 = cube_types::sentinel::void as u32;
pub const AIR: u32 = cube_types::object::air as u32;
pub const UNKNOWN: u32 = cube_types::sentinel::unknown as u32;
const _: () = assert!(VOID == 0);

/// The nodes a column's tree starts from: where Z takes its first digit.
const ROOT: Zoom = SHARD_ZOOM + 1;

const EMPTY: u16 = VOID as u16;

fn key(zoom: Zoom, at: [i64; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

/// A child of the node at `zoom`, its Z as its lane names it: the digits Z
/// has there, two's complement, so the first is its sign.
fn child(zoom: Zoom, at: [i64; 3], slot: usize) -> [i64; 3] {
    let [x, y, z] = child_of(at, slot);
    let w = 64 - u32::from(zoom + 1 - SHARD_ZOOM);
    [x, y, (z << w) >> w]
}

/// The column, a node at `SHARD_ZOOM`, a node is under.
fn column(zoom: Zoom, at: [i64; 3]) -> [i64; 3] {
    [at[0] >> (zoom - SHARD_ZOOM), at[1] >> (zoom - SHARD_ZOOM), 0]
}

/// Majority over the 2×2×2 children (pxl-spec bake/fold.rs), empty counted as a value
/// and a non-voxel value as empty, so a lone cube goes at the first fold and
/// a wall keeps whole cells.
fn fold_cell(children: [u16; 8], zoom: Zoom) -> u16 {
    majority(children.map(|v| if object_value_is_voxel(u32::from(v)) { v } else { EMPTY }), zoom)
}

/// One leaf written: `<key, id>`, its node and its catalog value. VOID
/// erases.
#[derive(Clone, Copy, Debug)]
pub struct Write {
    pub key: u128,
    pub id: u32,
}

/// The placed objects: one tree of leaves a column, nothing kept of what a
/// branch holds.
#[derive(Default)]
pub struct Store {
    columns: HashMap<u128, Tree, FnvBuildHasher>,
}

type Tree = SparseTree<u16, 3>;

/// A node met in a descent: its zoom, its index and cube in the asker's frame,
/// its value if it is a leaf; whether to go under it.
type Enter<'a> = dyn FnMut(Zoom, [i64; 3], &Aabb, Option<u16>) -> bool + 'a;

/// A node's X moved to the round nearest `about`.
fn moved(zoom: Zoom, at: [i64; 3], about: f64) -> [i64; 3] {
    if !about.is_finite() {
        return at;
    }
    let whole = cube(zoom, at);
    let x = (whole.min[0] + whole.max[0]) / 2.0;
    let rounds = ((nearest(x, about) - x) / ROUND).round() as i64;
    [at[0] + rounds * around(zoom), at[1], at[2]]
}

/// Where a node stands in its column's tree: the node or the leaf over it.
struct Found<'a> {
    tree: &'a Tree,
    zoom: Zoom,
    slot: Slot<u16>,
}

impl Store {
    fn find(&self, zoom: Zoom, at: [i64; 3]) -> Option<Found<'_>> {
        let tree = self.columns.get(&key(SHARD_ZOOM, column(zoom, at)))?;
        let (depth, slot) = tree.get(u32::from(zoom - SHARD_ZOOM), at);
        Some(Found {
            tree,
            zoom: SHARD_ZOOM + depth as Zoom,
            slot,
        })
    }

    /// What a node shows: a leaf's value, or its children folded.
    fn fold(&self, tree: &Tree, slot: Slot<u16>, zoom: Zoom, at: [i64; 3], hide: Option<u128>) -> u16 {
        match slot {
            Slot::Leaf(v) if hide == Some(key(zoom, at)) && v != EMPTY => EMPTY,
            Slot::Leaf(v) => v,
            Slot::Branch(block) => {
                let mut kids = [EMPTY; 8];
                for (k, s) in tree.block(block).enumerate() {
                    kids[k] = self.fold(tree, s, zoom + 1, child(zoom, at, k), hide);
                }
                fold_cell(kids, zoom)
            }
        }
    }

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        for (&k, tree) in &self.columns {
            let (_, col) = node_of::<3>(k);
            for (s, slot) in tree.block(0).enumerate() {
                self.walk(tree, slot, ROOT, child(SHARD_ZOOM, col, s), about, enter);
            }
        }
    }

    fn walk(&self, tree: &Tree, slot: Slot<u16>, zoom: Zoom, at: [i64; 3], about: f64, enter: &mut Enter) {
        let leaf = match slot {
            Slot::Leaf(EMPTY) => return,
            Slot::Leaf(v) => Some(v),
            Slot::Branch(_) => None,
        };
        let here = moved(zoom, at, about);
        if !enter(zoom, here, &cube(zoom, here), leaf) {
            return;
        }
        if let Slot::Branch(block) = slot {
            for (k, s) in tree.block(block).enumerate() {
                self.walk(tree, s, zoom + 1, child(zoom, at, k), about, enter);
            }
        }
    }

    /// Every leaf, as the lanes name it.
    pub fn each(&self, visit: &mut dyn FnMut(&Entry)) {
        self.descend(f64::NAN, &mut |zoom, at, _, leaf| {
            if let Some(v) = leaf {
                visit(&Entry { zoom, at, state: u32::from(v) });
            }
            true
        });
    }

    /// Every leaf whose cube meets `query`, X in `query`'s frame.
    pub fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.descend(about(query), &mut |zoom, at, whole, leaf| {
            if whole.meet(*query).is_none() {
                return false;
            }
            if let Some(v) = leaf {
                visit(&Entry { zoom, at, state: u32::from(v) });
            }
            true
        });
    }

    /// A box holding every entry inside `query`, cut to it, never none where
    /// there is some.
    pub fn bound_in(&self, query: &Aabb) -> Option<Aabb> {
        let mut out: Option<Aabb> = None;
        self.entries_in(query, &mut |e| {
            if let Some(b) = e.cube().meet(*query) {
                out = Some(out.map_or(b, |o| o.join(b)));
            }
        });
        out
    }

    /// The finest zoom any entry inside `query` is held at.
    pub fn finest_in(&self, query: &Aabb) -> Option<Zoom> {
        let mut out: Option<Zoom> = None;
        self.entries_in(query, &mut |e| out = Some(out.map_or(e.zoom, |f| f.max(e.zoom))));
        out
    }

    /// Every leaf meeting the cube at `zoom`, X in the cube's frame.
    pub fn each_in(&self, zoom: Zoom, at: [i64; 3], visit: &mut dyn FnMut(&Entry)) {
        let Some(f) = self.find(zoom, at) else { return };
        let about = about(&cube(zoom, at));
        match f.slot {
            Slot::Leaf(EMPTY) => {}
            Slot::Leaf(v) => visit(&Entry {
                zoom: f.zoom,
                at: moved(f.zoom, at.map(|c| c >> (zoom - f.zoom)), about),
                state: u32::from(v),
            }),
            branch => self.walk(f.tree, branch, zoom, at, about, &mut |zoom, at, _, leaf| {
                if let Some(v) = leaf {
                    visit(&Entry { zoom, at, state: u32::from(v) });
                }
                true
            }),
        }
    }

    /// Whether anything, air included, is held in the cube at `zoom`.
    pub fn holds(&self, zoom: Zoom, at: [i64; 3]) -> bool {
        self.find(zoom, at).is_some_and(|f| f.slot != Slot::Leaf(EMPTY))
    }

    /// The cell over the highest entry in the column of ground cell `[x, y]`.
    pub fn ceiling(&self, [x, y]: [i64; 2]) -> Option<i64> {
        let side = units(GROUND.cell());
        let column = Aabb {
            min: [x as f64 * side, y as f64 * side, REACH.min[2]],
            max: [(x + 1) as f64 * side, (y + 1) as f64 * side, REACH.max[2]],
        };
        self.bound_in(&column).map(|b| (b.max[2] / side).ceil() as i64)
    }

    /// Every cell of `cell_zoom` that may hold objects along the ray, in ray
    /// order, until `each` stops it: the 3D roots' cubes it crosses in turn,
    /// and in each the children it meets nearest first.
    pub fn along_ray(&self, origin: V3, direction: V3, span: (f64, f64), cell_zoom: Zoom, each: &mut dyn FnMut([i64; 3]) -> bool) {
        let ray = (origin, direction);
        let side = units(cell_zoom);
        cells_along(ray, span, units(SHARD_ZOOM), &mut |[x, y, half]| {
            let Some(tree) = self.columns.get(&key(SHARD_ZOOM, [x, y, 0])) else { return false };
            let mut met = [(0.0, Slot::Leaf(EMPTY), [0; 3]); 8];
            let mut n = 0;
            for (s, slot) in tree.block(0).enumerate() {
                let at = child(SHARD_ZOOM, [x, y, 0], s);
                if slot == Slot::Leaf(EMPTY) || at[2] != half {
                    continue;
                }
                if let Some(c) = slab(ray, &cube(ROOT, at).snapped_out(side), span) {
                    met[n] = (c.0, slot, at);
                    n += 1;
                }
            }
            met[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            met[..n].iter().any(|&(_, slot, at)| self.along(tree, slot, ray, span, cell_zoom, ROOT, at, each))
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn along(&self, tree: &Tree, slot: Slot<u16>, ray: (V3, V3), span: (f64, f64), cell_zoom: Zoom, zoom: Zoom, at: [i64; 3], each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
        let side = units(cell_zoom);
        let here = moved(zoom, at, ray.0[0]);
        let Some(s) = slab(ray, &cube(zoom, here).snapped_out(side), span) else { return false };
        if zoom == cell_zoom {
            return each(here);
        }
        let block = match slot {
            Slot::Leaf(EMPTY) => return false,
            // A leaf coarser than the cell is every cell of it.
            Slot::Leaf(_) => return cells_along(ray, s, side, each),
            Slot::Branch(block) => block,
        };
        let mut met: [(f64, Slot<u16>, [i64; 3]); 8] = [(0.0, Slot::Leaf(EMPTY), [0; 3]); 8];
        let mut n = 0;
        for (k, c) in tree.block(block).enumerate() {
            let at = child(zoom, at, k);
            if c == Slot::Leaf(EMPTY) {
                continue;
            }
            if let Some(cs) = slab(ray, &cube(zoom + 1, moved(zoom + 1, at, ray.0[0])).snapped_out(side), s) {
                met[n] = (cs.0, c, at);
                n += 1;
            }
        }
        met[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        met[..n].iter().any(|&(_, c, at)| self.along(tree, c, ray, s, cell_zoom, zoom + 1, at, each))
    }

    /// What the objects fold to in the cells `from..to` of `target`, at every
    /// height, with `hide` left out.
    pub fn fold_at(&self, target: Zoom, from: [i64; 2], to: [i64; 2], hide: Option<EntryAt>, out: &mut HashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        out.clear();
        let mut refused = Ok(());
        let hide = hide.map(|h| key(h.zoom(), h.at()));
        let inside = |x: i64, y: i64| (from[0]..to[0]).contains(&x) && (from[1]..to[1]).contains(&y);
        let side = units(target);
        let query = Aabb {
            min: [from[0] as f64 * side, from[1] as f64 * side, REACH.min[2]],
            max: [to[0] as f64 * side, to[1] as f64 * side, REACH.max[2]],
        };
        self.descend(about(&query), &mut |zoom, at, whole, leaf| {
            if whole.meet(query).is_none() || refused.is_err() {
                return false;
            }
            let leaf = leaf.filter(|_| hide != Some(key(zoom, at)));
            if zoom < target && leaf.is_none() {
                return true;
            }
            let v = u32::from(match leaf {
                Some(v) => v,
                None => self.find(zoom, at).map_or(EMPTY, |f| self.fold(f.tree, f.slot, f.zoom, at, hide)),
            });
            if !object_value_is_voxel(v) {
                return false;
            }
            // A leaf coarser than the cell is each of its cells.
            let cells = i64::from(count(target, zoom));
            let [x0, y0, z0] = first_descendant_of(at, u32::from(target - zoom));
            for dz in 0..cells {
                for y in (y0..y0 + cells).filter(|&y| (from[1]..to[1]).contains(&y)) {
                    for x in (x0..x0 + cells).filter(|&x| inside(x, y)) {
                        if refused.is_ok() {
                            refused = try_insert(out, [x, y, z0 + dz], v);
                        }
                    }
                }
            }
            false
        });
        refused
    }

    /// The leaf holding the cube at `zoom`, X in the asker's frame, or the
    /// cube itself as its eight fold to, and whether it is that fold.
    pub fn node(&self, zoom: Zoom, at: [i64; 3]) -> (Entry, bool) {
        let empty = (Entry { zoom, at, state: VOID }, false);
        let Some(f) = self.find(zoom, at) else { return empty };
        match f.slot {
            Slot::Leaf(EMPTY) => empty,
            Slot::Leaf(v) => (
                Entry {
                    zoom: f.zoom,
                    at: at.map(|c| c >> (zoom - f.zoom)),
                    state: u32::from(v),
                },
                false,
            ),
            branch => (
                Entry {
                    zoom,
                    at,
                    state: u32::from(self.fold(f.tree, branch, zoom, at, None)),
                },
                true,
            ),
        }
    }

    /// VOID where nothing; a leaf's value as it is.
    pub fn folded_value(&self, zoom: Zoom, at: [i64; 3]) -> u16 {
        self.find(zoom, at).map_or(EMPTY, |f| self.fold(f.tree, f.slot, f.zoom, at.map(|c| c >> (zoom - f.zoom)), None))
    }

    /// The value placed at exactly this node, if one is.
    fn placed(&self, zoom: Zoom, at: [i64; 3]) -> Option<u16> {
        let f = self.find(zoom, at)?;
        match f.slot {
            Slot::Leaf(v) if f.zoom == zoom && v != EMPTY => Some(v),
            _ => None,
        }
    }

    /// Applies every write; the cubes that changed.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> Vec<Aabb> {
        let mut changed = Vec::new();
        for Write { key: k, id } in writes {
            let (zoom, at) = node_of::<3>(k);
            xpute_core::ensure!((ROOT..=OBJECT_FINEST).contains(&zoom), ENOTRECOVERABLE, zoom);
            let value = u16::try_from(id).or_bug(Errno::EOVERFLOW);
            let col = key(SHARD_ZOOM, column(zoom, at));
            let tree = match self.columns.get_mut(&col) {
                Some(tree) => tree,
                None if value == EMPTY => continue,
                None => self.columns.entry(col).or_insert_with(|| Tree::new(EMPTY)),
            };
            if tree.set(u32::from(zoom - SHARD_ZOOM), at, value) {
                changed.push(cube(zoom, at));
            }
            if tree.uniform() == Some(EMPTY) {
                self.columns.remove(&col);
            }
        }
        changed
    }

    /// Empties the store; the cubes that held anything.
    pub fn clear(&mut self) -> Vec<Aabb> {
        let mut held = Vec::new();
        self.each(&mut |e| held.push(e.cube()));
        self.columns.clear();
        held
    }
}

fn about(b: &Aabb) -> f64 {
    (b.min[0] + b.max[0]) / 2.0
}

fn slab((o, d): (V3, V3), b: &Aabb, (t0, t1): (f64, f64)) -> Option<(f64, f64)> {
    let (enter, leave, _) = b.span_along(o, d)?;
    let (enter, leave) = (enter.max(t0), leave.min(t1));
    (enter <= leave).then_some((enter, leave))
}

/// Amanatides–Woo traversal of `side` cells; returns whether `each` stopped it.
fn cells_along((o, d): (V3, V3), (t0, t1): (f64, f64), side: f64, each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
    let start = [0, 1, 2].map(|k| o[k] + d[k] * t0);
    let mut cell = start.map(|v| (v / side).floor() as i64);
    let mut next = [0.0; 3];
    let mut delta = [0.0; 3];
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            (next[k], delta[k]) = (f64::INFINITY, f64::INFINITY);
        } else {
            let face = (cell[k] + i64::from(d[k] > 0.0)) as f64 * side;
            (next[k], delta[k]) = ((face - o[k]) / d[k], side / d[k].abs());
        }
    }
    loop {
        let k = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b])).unwrap_or(0);
        if each(cell) {
            return true;
        }
        if next[k] >= t1 {
            return false;
        }
        cell[k] += if d[k] > 0.0 { 1 } else { -1 };
        next[k] += delta[k];
    }
}

fn try_insert(out: &mut HashMap<[i64; 3], u32>, k: [i64; 3], v: u32) -> Result<(), TryReserveError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))?;
    }
    out.insert(k, v);
    Ok(())
}

/// A node's objects at a cell zoom, as the query reads them: the store and
/// what was asked, folded when read, nothing kept.
#[derive(Clone, Copy)]
pub struct Cells<'a> {
    store: &'a Store,
    node: Node<3>,
    cell: Zoom,
}

impl Cells<'_> {
    /// What the objects fold to in the cells `from..to` at the asked zoom, with `hide` left out.
    pub fn fold(&self, from: [i64; 2], to: [i64; 2], hide: Option<EntryAt>, out: &mut HashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        self.store.fold_at(self.cell, from, to, hide, out)
    }

    /// Every leaf meeting the node's cube.
    pub fn entries(&self, visit: &mut dyn FnMut(&Entry)) {
        self.store.entries_in(&cube(self.node.0, self.node.1), visit);
    }
}

impl Query for Store {
    type Record<'a> = Cells<'a>;

    /// All resident, so every answer is exact. Read at `cell`: a node held
    /// finer than its cells is one cube and `finest` one past `cell`, so the
    /// cost is the asker's resolution — one find for a node read whole.
    fn bounds<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let mut out: Option<Bound> = None;
        let mut hold = |aabb: Aabb, finest: Zoom| {
            out = Some(out.map_or(Bound { aabb, finest }, |o| Bound {
                aabb: o.aabb.join(aabb),
                finest: o.finest.max(finest),
            }))
        };
        let whole = region(node);
        // One tree holds a cube under a column's root; else its columns' leaves.
        let Some(at) = (D == 3 && node.0 >= SHARD_ZOOM).then(|| [node.1[0], node.1[1], node.1[2]]) else {
            self.entries_in(&whole, &mut |e| {
                if let Some(b) = e.cube().meet(whole) {
                    hold(b, e.zoom.min(cell + 1));
                }
            });
            return Answer::Exact(out);
        };
        let zoom = node.0;
        match self.find(zoom, at) {
            None => {}
            Some(Found { slot: Slot::Leaf(EMPTY), .. }) => {}
            Some(Found { slot: Slot::Leaf(_), zoom: z, .. }) => hold(whole, z),
            Some(f) => self.walk(f.tree, f.slot, zoom, at, about(&whole), &mut |z, _, c, leaf| {
                if leaf.is_some() {
                    hold(*c, z);
                    false
                } else if z >= cell {
                    hold(*c, cell + 1);
                    false
                } else {
                    true
                }
            }),
        }
        Answer::Exact(out)
    }

    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Cells<'_>> {
        Answer::Exact(Cells { store: self, node, cell })
    }
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
