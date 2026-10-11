// pxl-kernel/layer/object/store.rs

//! Placed objects, kept as their one record: each leaf's catalog value at its
//! node and the handle of what it carries beyond it (`ext.rs`), in one sparse
//! tree a shard (`space/hierarchy/sparse_tree.rs`). A write arrives as
//! `<key, id, ext>` (`morton::key_of`). What a node at any zoom shows is read
//! from the leaves over and under it, folding by the objects' majority, and
//! never kept. A leaf that carries a record is never joined, its handle its
//! own, and is never split: a placement takes an empty cell and an erase
//! takes the whole leaf.

use fnv::FnvHashMap;
use xpute_guest::global::Global;

use std::collections::TryReserveError;

pub use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::fold::majority;
use pxl_spec::block::object::{cube_of, decor_id_from_num, object_value_is_decor, object_value_is_marker, object_value_is_voxel, Cube};
use pxl_spec::block::{cube, DecorId};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::world::{around, node_near, wrap_index, REACH};
use pxl_spec::space::coord::zoom::{base_of, count, cube, index_of, of_side, units, Zoom, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{child_cube, key_of, node_of};
use pxl_spec::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_spec::space::hierarchy::tree::{first_descendant_of, region, Node};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::kernel;
use crate::layer::object::ext::{Ext, Pools, NONE};
use crate::layer::{Answer, Bound, Query};
use crate::sched::edge::request_world_tick;
use pxl_spec::block::ext::class_of;

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Cube { cube: Cube },
    Decor { decor_id: DecorId },
    Marker,
}

/// A leaf as an asker meets it: its cube, X in the asker's frame, and the
/// handle of its record in its class's pool.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    pub zoom: Zoom,
    pub at: [i64; 3],
    pub state: u32,
    pub ext: u16,
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

    /// What it is, by its kind; not for air, which is the eraser's mark.
    pub fn block(&self) -> Block {
        if object_value_is_marker(self.state) {
            Block::Marker
        } else if object_value_is_decor(self.state) {
            Block::Decor {
                decor_id: decor_id_from_num(self.state),
            }
        } else {
            Block::Cube {
                // Every value the store writes is one the catalog gives out.
                cube: cube_of(self.state).or_bug(Errno::ENOTRECOVERABLE),
            }
        }
    }
}

pub(crate) fn apply(writes: impl IntoIterator<Item = Write>) {
    let regions = kernel().objects.write(writes);
    if !regions.is_empty() {
        written(&regions);
    }
}

static WRITTEN: Global<Vec<Aabb>> = Global::new();

/// Where the objects changed, kept for the turn to tell what draws them and
/// what is worked out from them (`take_written`); storage calls nothing above it.
pub(super) fn written(regions: &[Aabb]) {
    WRITTEN.get(Vec::new).extend_from_slice(regions);
    request_world_tick();
}

/// The regions written since the turn last took them, into `out`.
pub fn take_written(out: &mut Vec<Aabb>) {
    out.clear();
    core::mem::swap(out, WRITTEN.get(Vec::new));
}

pub const VOID: u32 = cube::sentinel::void as u32;
pub const AIR: u32 = cube::object::air as u32;
pub const UNKNOWN: u32 = cube::sentinel::unknown as u32;
const _: () = assert!(VOID == 0);

/// What a tree's leaf holds: the catalog value, and its record's handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaf {
    pub id: u16,
    pub ext: u16,
}

const EMPTY: Leaf = Leaf { id: VOID as u16, ext: NONE };

fn key(zoom: Zoom, at: [i64; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

/// The shard a node is under.
fn shard(zoom: Zoom, at: [i64; 3]) -> [i64; 3] {
    [at[0] >> (zoom - SHARD_ZOOM), at[1] >> (zoom - SHARD_ZOOM), 0]
}

/// Majority over the 2×2×2 children (pxl-spec bake/fold.rs), empty counted as a value
/// and a non-voxel value as empty, so a lone cube goes at the first fold and
/// a wall keeps whole cells.
fn fold_cell(children: [u16; 8], zoom: Zoom) -> u16 {
    majority(children.map(|v| if object_value_is_voxel(u32::from(v)) { v } else { EMPTY.id }), zoom)
}

/// One leaf written: `<key, id, ext>`, its node, its catalog value and what
/// it carries. VOID erases.
#[derive(Clone, Debug)]
pub struct Write {
    pub key: u128,
    pub id: u32,
    pub ext: Option<Ext>,
}

impl Write {
    /// `id` at the cube of `size` holding `coord`. Size is a power of two by
    /// construction; the nearest zoom only guards against float noise.
    pub fn at(coord: &Pvf3, size: f64, id: u32) -> Write {
        let zoom = of_side(size);
        Write {
            key: key(zoom, index_of(zoom, coord)),
            id,
            ext: None,
        }
    }

    pub fn carrying(self, ext: Ext) -> Write {
        Write { ext: Some(ext), ..self }
    }
}

/// The placed objects: one tree of leaves a shard, nothing kept of what a
/// branch holds.
#[derive(Default)]
pub struct Store {
    shards: FnvHashMap<u128, Tree>,
    ext: Pools,
}

type Tree = SparseTree<Leaf, 3>;

/// A node met in a descent: its zoom, its index and cube in the asker's frame,
/// its value if it is a leaf; whether to go under it.
type Enter<'a> = dyn FnMut(Zoom, [i64; 3], &Aabb, Option<Leaf>) -> bool + 'a;

/// Where a node stands in its shard's tree: the node or the leaf over it.
struct Found<'a> {
    tree: &'a Tree,
    zoom: Zoom,
    slot: Slot<Leaf>,
}

impl Store {
    fn find(&self, zoom: Zoom, at: [i64; 3]) -> Option<Found<'_>> {
        let tree = self.shards.get(&key(SHARD_ZOOM, shard(zoom, at)))?;
        // The shard itself is its tree's root, whose children are the first block.
        if zoom == SHARD_ZOOM {
            return Some(Found { tree, zoom, slot: Slot::Branch(0) });
        }
        let (depth, slot) = tree.get(u32::from(zoom - SHARD_ZOOM), at);
        Some(Found {
            tree,
            zoom: SHARD_ZOOM + depth as Zoom,
            slot,
        })
    }

    /// What a node shows: a leaf's value, or its children folded.
    fn fold(&self, tree: &Tree, slot: Slot<Leaf>, zoom: Zoom, at: [i64; 3], hide: Option<u128>) -> u16 {
        match slot {
            Slot::Leaf(v) if hide == Some(key(zoom, at)) && v != EMPTY => EMPTY.id,
            Slot::Leaf(v) => v.id,
            Slot::Branch(block) => {
                let mut kids = [EMPTY.id; 8];
                for (k, s) in tree.block(block).enumerate() {
                    kids[k] = self.fold(tree, s, zoom + 1, child_cube(zoom, at, k), hide);
                }
                fold_cell(kids, zoom)
            }
        }
    }

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        for (&k, tree) in &self.shards {
            self.walk_column(tree, node_of::<3>(k).1, about, enter);
        }
    }

    /// As `descend`, over only the shards `region` meets, edges included:
    /// what is read is as large as what is asked, not as the world's edits.
    fn descend_in(&self, region: &Aabb, enter: &mut Enter) {
        let side = units(SHARD_ZOOM);
        let across = |k: usize| (region.min[k] / side).floor() as i64..=(region.max[k] / side).floor() as i64;
        for y in across(1) {
            for x in across(0) {
                let shard = [wrap_index(SHARD_ZOOM, x), y, 0];
                if let Some(tree) = self.shards.get(&key(SHARD_ZOOM, shard)) {
                    self.walk_column(tree, shard, about(region), enter);
                }
            }
        }
    }

    fn walk_column(&self, tree: &Tree, shard: [i64; 3], about: f64, enter: &mut Enter) {
        for (s, slot) in tree.block(0).enumerate() {
            self.walk(tree, slot, CUBE_ZOOM, child_cube(SHARD_ZOOM, shard, s), about, enter);
        }
    }

    fn walk(&self, tree: &Tree, slot: Slot<Leaf>, zoom: Zoom, at: [i64; 3], about: f64, enter: &mut Enter) {
        let leaf = match slot {
            Slot::Leaf(EMPTY) => return,
            Slot::Leaf(v) => Some(v),
            Slot::Branch(_) => None,
        };
        let here = node_near(zoom, at, about);
        if !enter(zoom, here, &cube(zoom, here), leaf) {
            return;
        }
        if let Slot::Branch(block) = slot {
            for (k, s) in tree.block(block).enumerate() {
                self.walk(tree, s, zoom + 1, child_cube(zoom, at, k), about, enter);
            }
        }
    }

    /// Every leaf, as the lanes name it.
    pub fn each(&self, visit: &mut dyn FnMut(&Entry)) {
        self.descend(f64::NAN, &mut |zoom, at, _, leaf| {
            if let Some(v) = leaf {
                visit(&Entry::of(zoom, at, v));
            }
            true
        });
    }

    /// Every leaf whose cube meets `query`, X in `query`'s frame.
    pub fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.descend_in(query, &mut |zoom, at, whole, leaf| {
            if whole.meet(*query).is_none() {
                return false;
            }
            if let Some(v) = leaf {
                visit(&Entry::of(zoom, at, v));
            }
            true
        });
    }

    /// Every leaf meeting the cube at `zoom`, X in the cube's frame.
    fn each_in(&self, zoom: Zoom, at: [i64; 3], visit: &mut dyn FnMut(&Entry)) {
        let Some(f) = self.find(zoom, at) else { return };
        let about = about(&cube(zoom, at));
        match f.slot {
            Slot::Leaf(EMPTY) => {}
            Slot::Leaf(v) => visit(&Entry::of(f.zoom, node_near(f.zoom, at.map(|c| c >> (zoom - f.zoom)), about), v)),
            branch => self.walk(f.tree, branch, zoom, at, about, &mut |zoom, at, _, leaf| {
                if let Some(v) = leaf {
                    visit(&Entry::of(zoom, at, v));
                }
                true
            }),
        }
    }

    /// Whether anything, air included, is held in the cube at `zoom`.
    fn holds(&self, zoom: Zoom, at: [i64; 3]) -> bool {
        self.find(zoom, at).is_some_and(|f| f.slot != Slot::Leaf(EMPTY))
    }

    /// What the objects fold to in the cells `from..to` of `target`, at every
    /// height, with `hide` left out.
    fn fold_at(&self, target: Zoom, from: [i64; 2], to: [i64; 2], hide: Option<EntryAt>, out: &mut FnvHashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        out.clear();
        let mut refused = Ok(());
        let hide = hide.map(|h| key(h.zoom(), h.at()));
        let inside = |x: i64, y: i64| (from[0]..to[0]).contains(&x) && (from[1]..to[1]).contains(&y);
        let side = units(target);
        let query = Aabb {
            min: [from[0] as f64 * side, from[1] as f64 * side, REACH.min[2]],
            max: [to[0] as f64 * side, to[1] as f64 * side, REACH.max[2]],
        };
        self.descend_in(&query, &mut |zoom, at, whole, leaf| {
            if whole.meet(query).is_none() || refused.is_err() {
                return false;
            }
            let leaf = leaf.filter(|_| hide != Some(key(zoom, at)));
            if zoom < target && leaf.is_none() {
                return true;
            }
            let v = u32::from(match leaf {
                Some(v) => v.id,
                None => self.find(zoom, at).map_or(EMPTY.id, |f| self.fold(f.tree, f.slot, f.zoom, at, hide)),
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
        let empty = (Entry { zoom, at, state: VOID, ext: NONE }, false);
        let Some(f) = self.find(zoom, at) else { return empty };
        match f.slot {
            Slot::Leaf(EMPTY) => empty,
            Slot::Leaf(v) => (Entry::of(f.zoom, at.map(|c| c >> (zoom - f.zoom)), v), false),
            branch => (
                Entry {
                    zoom,
                    at,
                    state: u32::from(self.fold(f.tree, branch, zoom, at, None)),
                    ext: NONE,
                },
                true,
            ),
        }
    }

    /// The value placed at exactly this node, if one is.
    pub(crate) fn placed(&self, zoom: Zoom, at: [i64; 3]) -> Option<Entry> {
        let f = self.find(zoom, at)?;
        match f.slot {
            Slot::Leaf(v) if f.zoom == zoom && v != EMPTY => Some(Entry::of(zoom, at, v)),
            _ => None,
        }
    }

    /// What a leaf carries beyond its id, if anything.
    pub fn ext(&self, e: &Entry) -> Option<&Ext> {
        self.ext.get(class_of(e.state)?, e.ext)
    }

    /// A marker's words, as put up; none for anything else.
    pub fn words(&self, e: &Entry) -> Option<&str> {
        match self.ext(e)? {
            Ext::MarkerText(t) => Some(t),
        }
    }

    /// Applies every write; the cubes that changed. A write whose record its
    /// pool has no room for is left out, as if never sent.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> Vec<Aabb> {
        let mut changed = Vec::new();
        for Write { key: k, id, ext } in writes {
            let (zoom, at) = node_of::<3>(k);
            xpute_kit::ensure!((CUBE_ZOOM..=OBJECT_FINEST).contains(&zoom), ENOTRECOVERABLE, zoom);
            let id16 = u16::try_from(id).or_bug(Errno::EOVERFLOW);
            // A record is the shape its id's class names, or none.
            xpute_kit::ensure!(ext.as_ref().map(Ext::class) == ext.as_ref().and(class_of(id)), EINVAL, id);
            self.release(zoom, at);
            let handle = match ext {
                Some(ext) => match self.ext.keep(ext) {
                    Some(h) => h,
                    None => continue,
                },
                None => NONE,
            };
            let value = Leaf { id: id16, ext: handle };
            let shard = key(SHARD_ZOOM, shard(zoom, at));
            let tree = match self.shards.get_mut(&shard) {
                Some(tree) => tree,
                None if value == EMPTY => continue,
                None => self.shards.entry(shard).or_insert_with(|| Tree::new(EMPTY)),
            };
            if tree.set(u32::from(zoom - SHARD_ZOOM), at, value) {
                changed.push(cube(zoom, at));
            }
            if tree.uniform() == Some(EMPTY) {
                self.shards.remove(&shard);
            }
        }
        changed
    }

    /// Frees the records of the leaves the node at `zoom` replaces: a leaf
    /// at it or under it. One over it with a record is never split.
    fn release(&mut self, zoom: Zoom, at: [i64; 3]) {
        let mut freed = Vec::new();
        if let Some(f) = self.find(zoom, at) {
            if let Slot::Leaf(v) = f.slot {
                xpute_kit::ensure!(f.zoom == zoom || v.ext == NONE, ENOTRECOVERABLE, f.zoom);
            }
        }
        self.each_in(zoom, at, &mut |e| {
            if e.ext != NONE {
                freed.push((e.state, e.ext));
            }
        });
        for (id, h) in freed {
            if let Some(class) = class_of(id) {
                self.ext.free(class, h);
            }
        }
    }

    /// Empties the store; the cubes that held anything.
    pub fn clear(&mut self) -> Vec<Aabb> {
        let mut held = Vec::new();
        self.each(&mut |e| held.push(e.cube()));
        self.shards.clear();
        self.ext.clear();
        held
    }
}

impl Entry {
    fn of(zoom: Zoom, at: [i64; 3], leaf: Leaf) -> Entry {
        Entry {
            zoom,
            at,
            state: u32::from(leaf.id),
            ext: leaf.ext,
        }
    }
}

fn about(b: &Aabb) -> f64 {
    (b.min[0] + b.max[0]) / 2.0
}

fn try_insert(out: &mut FnvHashMap<[i64; 3], u32>, k: [i64; 3], v: u32) -> Result<(), TryReserveError> {
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
    /// The cells, X and Y at the asked zoom, of the node's shard and `apron`
    /// more on each side.
    pub fn column_cells(&self, apron: i64) -> ([i64; 2], [i64; 2]) {
        let (zoom, at) = self.node;
        let [x, y, _] = first_descendant_of(at, u32::from(self.cell - zoom));
        let n = i64::from(count(self.cell, zoom));
        ([x - apron, y - apron], [x + n + apron, y + n + apron])
    }

    /// What the objects fold to over the node's shard and `apron` more cells
    /// on each side, at the asked zoom, with `hide` left out.
    pub fn fold(&self, apron: i64, hide: Option<EntryAt>, out: &mut FnvHashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        let (from, to) = self.column_cells(apron);
        self.store.fold_at(self.cell, from, to, hide, out)
    }

    /// Cuts the cells `carve` covers to air in `out`, as `fold` writes them:
    /// an eraser's preview.
    pub fn carve(&self, apron: i64, carve: EntryAt, out: &mut FnvHashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        let (from, _) = self.column_cells(apron);
        let own = carve.zoom();
        let [ix, iy, iz] = index_of(self.cell, &base_of(own, carve.at()));
        let ix = from[0] + (ix - from[0]).rem_euclid(around(self.cell));
        let n = if own < self.cell { i64::from(count(self.cell, own)) } else { 1 };
        for dz in 0..n {
            for dy in 0..n {
                for dx in 0..n {
                    try_insert(out, [ix + dx, iy + dy, iz + dz], AIR)?;
                }
            }
        }
        Ok(())
    }

    /// The leaf holding the node, or the node as its eight fold to, and whether it is that fold.
    pub fn node(&self) -> (Entry, bool) {
        self.store.node(self.node.0, self.node.1)
    }

    /// Every leaf inside the node, or the one over it.
    pub fn entries(&self, visit: &mut dyn FnMut(&Entry)) {
        let (zoom, at) = self.node;
        if zoom >= SHARD_ZOOM {
            return self.store.each_in(zoom, at, visit);
        }
        // Over a tree's root: the leaves of every shard under it, those
        // only touching its faces left out.
        let whole = cube(zoom, at);
        self.store.entries_in(&whole, &mut |e| {
            if e.cube().meet(whole).is_some_and(|m| (0..3).all(|k| m.min[k] < m.max[k])) {
                visit(e);
            }
        });
    }

    /// Whether anything, air included, is held at the node.
    pub fn holds(&self) -> bool {
        self.store.holds(self.node.0, self.node.1)
    }

    /// The leaf placed at exactly the node, if one is.
    pub fn placed(&self) -> Option<Entry> {
        self.store.placed(self.node.0, self.node.1)
    }
}

impl Query for Store {
    type Record<'a> = Cells<'a>;
    type Cursor = ();

    fn cursor<const D: usize>(&self, _: Node<D>, _: Zoom) {}

    fn child<const D: usize>(&self, _: &(), _: Node<D>, _: Zoom) {}

    /// All resident, so every answer is exact. Read at `cell`: a node held
    /// finer than its cells is one cube and `finest` one past `cell`, so the
    /// cost is the asker's resolution — one find for a node read whole.
    fn bound<const D: usize>(&self, _: &(), node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let mut out: Option<Bound> = None;
        let mut hold = |aabb: Aabb, finest: Zoom| {
            out = Some(out.map_or(Bound { aabb, finest }, |o| Bound {
                aabb: o.aabb.join(aabb),
                finest: o.finest.max(finest),
            }))
        };
        let whole = region(node);
        // One tree holds a cube under a shard's root; else its shards' leaves.
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
