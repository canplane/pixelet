// pxl-kernel/layer/object/store.rs

//! Placed objects, kept as their one record: each leaf's catalog value at its
//! node and the handle of what it carries beyond it (`ext.rs`), in one sparse
//! tree a shard (`space/hierarchy/sparse_tree.rs`). A write arrives as
//! `<key, id, ext>` (`morton::key_of`). What a node at any zoom holds is the
//! pyramid's (`pyramid.rs`), derived from this record beside it and refolded
//! by each write over the nodes it changes, read through the layer's query
//! (`Query::cursor`, `Query::child`, `Store::value`). A leaf that carries a
//! record is never joined, its handle its own, and is never split: a
//! placement takes an empty cell and an erase takes the whole leaf.

use fnv::FnvHashMap;
use xpute_guest::global::Global;

pub use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::block::object::{cube_of, decor_id_from_num, object_value_is_decor, object_value_is_marker, Cube};
use pxl_spec::block::{cube, DecorId};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::{node_near, wrap_index};
use pxl_spec::space::coord::zoom::{base_of, cube, index_of, of_side, units, Zoom, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{child_cube, key_of, node_of};
use pxl_spec::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_spec::space::hierarchy::tree::{region, Node};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::kernel;
use crate::layer::object::ext::{Ext, Pools, NONE};
use crate::layer::object::pyramid::{Held, Pyramid};
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

    pub fn corner(&self) -> Q3 {
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

pub(super) const EMPTY: Leaf = Leaf { id: VOID as u16, ext: NONE };

pub(super) fn key(zoom: Zoom, at: [i64; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

/// The shard a node is under.
pub(super) fn shard(zoom: Zoom, at: [i64; 3]) -> [i64; 3] {
    [at[0] >> (zoom - SHARD_ZOOM), at[1] >> (zoom - SHARD_ZOOM), 0]
}

/// The 2×2×2 children folded (pxl-spec bake/fold.rs): the share of the cube
/// filled, the mean of theirs, and the kind most of that fill is, a tie to
/// the corner `majority`'s alternates to. A non-voxel value fills nothing.
pub(super) fn fold_cell(children: [(u16, f64); 8], zoom: Zoom) -> (u16, f64) {
    let fill = children.iter().map(|c| c.1).sum::<f64>() / 8.0;
    let of = |kind: u16| children.iter().filter(|c| c.0 == kind).map(|c| c.1).sum::<f64>();
    let mut order = children;
    if zoom & 1 == 0 {
        order.reverse();
    }
    let mut best = (EMPTY.id, 0.0);
    for c in order {
        let w = of(c.0);
        if c.1 > 0.0 && w > best.1 {
            best = (c.0, w);
        }
    }
    (best.0, fill)
}

/// What a folded cube shows: its kind where it is half full or more, so a
/// lone cube goes at the first fold and a wall keeps whole cells, and the
/// volume is rounded once, where it is drawn, not at every zoom on the way.
pub(super) fn shown((kind, fill): (u16, f64)) -> u16 {
    if fill >= 0.5 {
        kind
    } else {
        EMPTY.id
    }
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
    pub fn at(coord: &Q3, size: f64, id: u32) -> Write {
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
    /// What each node holds at every zoom, derived from `shards`.
    pyramid: Pyramid,
}

pub(super) type Tree = SparseTree<Leaf, 3>;

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

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        for (&k, tree) in &self.shards {
            self.walk_shard(tree, node_of::<3>(k).1, about, enter);
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
                    self.walk_shard(tree, shard, about(region), enter);
                }
            }
        }
    }

    fn walk_shard(&self, tree: &Tree, shard: [i64; 3], about: f64, enter: &mut Enter) {
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
            self.pyramid.forget(self.shards.get(&shard), zoom, at);
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
            self.pyramid.refold(self.shards.get(&shard), zoom, at);
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
        self.pyramid.clear();
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

/// A node's leaves at a cell zoom, as the query reads them: the store and
/// what was asked. What the node holds folded is the pyramid's
/// (`Query::cursor`, `Store::value`).
#[derive(Clone, Copy)]
pub struct Cells<'a> {
    store: &'a Store,
    node: Node<3>,
}

impl Cells<'_> {
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
    /// What a cube holds, its parent's handed down (`pyramid.rs` `Held`);
    /// a node over the ground's two axes is read under it.
    type Cursor = Held;

    fn cursor<const D: usize>(&self, (zoom, at): Node<D>, _: Zoom) -> Held {
        match D {
            3 => self.pyramid.held(zoom, [at[0], at[1], at[2]]),
            _ => Held::Folded(EMPTY.id, 0.0),
        }
    }

    fn child<const D: usize>(&self, parent: &Held, (zoom, at): Node<D>, _: Zoom) -> Held {
        match D {
            3 => self.pyramid.child(*parent, zoom, [at[0], at[1], at[2]]),
            _ => *parent,
        }
    }

    /// All resident, so every answer is exact. Read at `cell`: a node held
    /// finer than its cells is one cube and `finest` one past `cell`, so the
    /// cost is the asker's resolution — one find for a node read whole.
    fn bound<const D: usize>(&self, held: &Held, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        if *held == Held::Nothing {
            return Answer::Exact(None);
        }
        let mut out: Option<Bound> = None;
        let mut hold = |aabb: Aabb, finest: Zoom| out = Bound::join(out, Some(Bound { aabb, finest }));
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

    fn read(&self, node: Node<3>, _: Zoom) -> Answer<Cells<'_>> {
        Answer::Exact(Cells { store: self, node })
    }
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
