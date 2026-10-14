// pxl-kernel/layer/object/store.rs

//! Placed objects, kept as their one record: each leaf's catalog value at its
//! node and the handle of what it carries beyond it (`ext.rs`), in one sparse
//! tree over the world (`space/hierarchy/sparse_tree.rs`), whose root's
//! children are the nodes of zoom 0. A write arrives as `<key, id, ext>`
//! (`morton::key_of`). What a node at any zoom holds is the pyramid's
//! (`pyramid.rs`), each branch's slot its children folded, folded again by
//! each write up its path, read through the layer's query (`Query::cursor`,
//! `Query::child`, `Store::value`). A leaf that carries a record is never
//! joined, its handle its own, and is never split: a placement takes an
//! empty cell and an erase takes the whole leaf.

use xpute_guest::global::Global;

pub use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::block::object::{cube_of, decor_id_from_num, object_value_is_decor, object_value_is_marker, Cube};
use pxl_spec::block::{cube, DecorId};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::{around, node_near, wrap_cell};
use pxl_spec::space::coord::zoom::{base_of, cube, index_of, of_side, units, Zoom, CUBE_ZOOM};
use pxl_spec::space::hierarchy::morton::{key_of, node_of};
use pxl_spec::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_spec::space::hierarchy::tree::{child_of, region, Node};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::kernel;
use crate::layer::object::ext::{Ext, Pools, NONE};
use crate::layer::object::pyramid::{fold, Held};
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

impl Leaf {
    pub(super) fn of(word: u32) -> Leaf {
        Leaf {
            id: word as u16,
            ext: (word >> 16) as u16,
        }
    }

    /// Its word in the tree: the value low, the handle high.
    pub(super) const fn word(self) -> u32 {
        self.id as u32 | (self.ext as u32) << 16
    }
}

pub(super) fn key(zoom: Zoom, at: [i64; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

/// Levels under the tree's root of a node at `zoom`.
const fn depth(zoom: Zoom) -> u32 {
    zoom + 1
}

/// The node `at` of `zoom` as the tree is cut: X counted from the round's
/// start, so the round is the one node of zoom 0 over it and every node is
/// its parent's index doubled, a bit more, on every axis.
fn path(zoom: Zoom, at: [i64; 3]) -> [i64; 3] {
    [at[0].rem_euclid(around(zoom)), at[1], at[2]]
}

/// The node of zoom 0 at the root's slot `k`.
fn root(k: usize) -> [i64; 3] {
    core::array::from_fn(|i| -(((k >> i) & 1) as i64))
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

/// The placed objects: one tree of leaves, each branch its children folded,
/// a word a slot.
pub struct Store {
    tree: Tree,
    ext: Pools,
}

impl Default for Store {
    fn default() -> Store {
        Store {
            tree: Tree::new(EMPTY.word()),
            ext: Pools::default(),
        }
    }
}

pub(super) type Tree = SparseTree<u32, 3>;

/// A node met in a descent: its zoom, its index and cube in the asker's frame,
/// its value if it is a leaf; whether to go under it.
type Enter<'a> = dyn FnMut(Zoom, [i64; 3], &Aabb, Option<Leaf>) -> bool + 'a;

impl Store {
    /// The node `at` of `zoom` in the tree, or the leaf over it: the zoom it
    /// was found at and its slot.
    fn find(&self, zoom: Zoom, at: [i64; 3]) -> (Zoom, Slot<u32>) {
        let (d, slot) = self.tree.get(depth(zoom), path(zoom, at));
        (d - 1, slot)
    }

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        for k in 0..8 {
            self.walk(self.tree.child(0, k), 0, root(k), about, enter);
        }
    }

    fn walk(&self, slot: Slot<u32>, zoom: Zoom, at: [i64; 3], about: f64, enter: &mut Enter) {
        let leaf = match slot {
            Slot::Leaf(v) if v == EMPTY.word() => return,
            Slot::Leaf(v) => Some(Leaf::of(v)),
            Slot::Branch(..) => None,
        };
        let here = node_near(zoom, at, about);
        if !enter(zoom, here, &cube(zoom, here), leaf) {
            return;
        }
        if let Slot::Branch(block, _) = slot {
            for k in 0..8 {
                self.walk(self.tree.child(block, k), zoom + 1, wrap_cell(zoom + 1, child_of(at, k)), about, enter);
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
        self.descend(about(query), &mut |zoom, at, whole, leaf| {
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
        let about = about(&cube(zoom, at));
        match self.find(zoom, at) {
            (_, Slot::Leaf(v)) if v == EMPTY.word() => {}
            (z, Slot::Leaf(v)) => visit(&Entry::of(z, node_near(z, at.map(|c| c >> (zoom - z)), about), Leaf::of(v))),
            (_, branch) => self.walk(branch, zoom, at, about, &mut |zoom, at, _, leaf| {
                if let Some(v) = leaf {
                    visit(&Entry::of(zoom, at, v));
                }
                true
            }),
        }
    }

    /// Whether anything, air included, is held in the cube at `zoom`.
    fn holds(&self, zoom: Zoom, at: [i64; 3]) -> bool {
        self.find(zoom, at).1 != Slot::Leaf(EMPTY.word())
    }

    /// The value placed at exactly this node, if one is.
    pub(crate) fn placed(&self, zoom: Zoom, at: [i64; 3]) -> Option<Entry> {
        match self.find(zoom, at) {
            (z, Slot::Leaf(v)) if z == zoom && v != EMPTY.word() => Some(Entry::of(zoom, at, Leaf::of(v))),
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
            let value = Leaf { id: id16, ext: handle }.word();
            if self.tree.set(depth(zoom), path(zoom, at), value, |tree, block, d| fold(tree, block, d - 1)) {
                changed.push(cube(zoom, at));
            }
        }
        changed
    }

    /// Frees the records of the leaves the node at `zoom` replaces: a leaf
    /// at it or under it. One over it with a record is never split.
    fn release(&mut self, zoom: Zoom, at: [i64; 3]) {
        let mut freed = Vec::new();
        if let (z, Slot::Leaf(v)) = self.find(zoom, at) {
            xpute_kit::ensure!(z == zoom || Leaf::of(v).ext == NONE, ENOTRECOVERABLE, z);
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
        self.tree = Tree::new(EMPTY.word());
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
        self.store.each_in(zoom, at, visit)
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
            3 => {
                let at = [at[0], at[1], at[2]];
                let (found, slot) = self.find(zoom, at);
                Held::of(slot, found, (zoom, at))
            }
            _ => Held::Folded {
                fold: Default::default(),
                block: 0,
            },
        }
    }

    fn child<const D: usize>(&self, parent: &Held, (zoom, at): Node<D>, _: Zoom) -> Held {
        match D {
            3 => parent.child(&self.tree, zoom, [at[0], at[1], at[2]]),
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
        // The tree holds a cube; a node over the ground's two axes, its leaves.
        let Some(at) = (D == 3).then(|| [node.1[0], node.1[1], node.1[2]]) else {
            self.entries_in(&whole, &mut |e| {
                if let Some(b) = e.cube().meet(whole) {
                    hold(b, e.zoom.min(cell + 1));
                }
            });
            return Answer::Exact(out);
        };
        let zoom = node.0;
        match self.find(zoom, at) {
            (_, Slot::Leaf(v)) if v == EMPTY.word() => {}
            (z, Slot::Leaf(_)) => hold(whole, z),
            (_, branch) => self.walk(branch, zoom, at, about(&whole), &mut |z, _, c, leaf| {
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
