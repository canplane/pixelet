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

use pxl_spec::bake::constant::{EntryAt, OBJECT_FINEST};
use pxl_spec::block::object::{cube_of, decor_id_from_num, object_value_is_decor, object_value_is_marker, Cube};
use pxl_spec::block::{cube, DecorId};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::{node_near, wrap_cell};
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom, CUBE_ZOOM};
use pxl_spec::space::hierarchy::morton::{key_of, node_of};
use pxl_spec::space::hierarchy::sparse_tree::SparseTree;
use pxl_spec::space::hierarchy::tree::{child_of, region, slot_of, Node};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::layer::object::ext::{Ext, Pools, NONE};
use crate::layer::{Answer, Query};
use crate::sched::edge::request_world_tick;
use pxl_spec::block::ext::class_of;
use pxl_spec::block::volume::fold;
use pxl_spec_core::overwrite::{self, path, Held};

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
    pub at: [i32; 3],
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

/// The placed objects, one store, the layer's own.
static OBJECTS: Global<Store> = Global::new();

pub fn objects() -> &'static mut Store {
    OBJECTS.get(Store::default)
}

pub(crate) fn apply(writes: impl IntoIterator<Item = Write>) {
    let regions = objects().write(writes);
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
            id: overwrite::low(word) as u16,
            ext: overwrite::high(word) as u16,
        }
    }

    /// Its word in the tree (pxl-spec-core `overwrite::leaf_word`).
    pub(super) const fn word(self) -> u32 {
        overwrite::leaf_word(self.id as u32, self.ext as u32)
    }
}

pub(super) fn key(zoom: Zoom, at: [i32; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

/// Levels under the tree's root of a node at `zoom`.
const fn depth(zoom: Zoom) -> u32 {
    zoom + 1
}

/// The tree's lanes as pxl-spec-core `overwrite` reads them, the CPU as a GPU.
pub struct Lanes<'a>(&'a Tree);

impl overwrite::Lanes for Lanes<'_> {
    fn heap(&self, slot: u32) -> u32 {
        self.0.heap()[slot as usize]
    }
    fn data(&self, slot: u32) -> u32 {
        self.0.data()[slot as usize]
    }
}

/// The root `k` of four, a node of zoom 0: X is counted from the round's
/// start (`overwrite::path`), so a root's X is 0, and Y and Z either side
/// of 0.
fn root(k: u32) -> [i32; 3] {
    [0, -((k & 1) as i32), -((k >> 1) as i32)]
}

/// Where the node `at` of `zoom` comes in the tree's own order, the order a
/// walk from the roots enters nodes in (`descend`): its slot a zoom
/// (`slot_of`) from the roots down, so of two nodes neither under the other
/// the one entered first is the lesser.
pub(crate) fn tree_key(zoom: Zoom, at: [i32; 3]) -> u128 {
    let p = path(zoom, at);
    let mut key = 0u128;
    for l in 0..=zoom {
        key = key << 3 | slot_of(p.map(|v| v >> (zoom - l))) as u128;
    }
    key << (3 * (OBJECT_FINEST - zoom))
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
    /// `id` at the cube `at` names.
    pub fn at(at: EntryAt, id: u32) -> Write {
        Write {
            key: key(at.zoom(), at.at()),
            id,
            ext: None,
        }
    }
}

/// The placed objects: one tree of leaves, each branch its children folded,
/// a word a slot.
pub struct Store {
    tree: Tree,
    ext: Pools,
    /// Bumped by every write that changes the tree, so a reader holding a
    /// copy of its lanes, the GPU's, knows to take them again.
    revision: u32,
    /// The blocks the tree wrote between the revision the lanes were last
    /// handed at (`since`) and that hand (`gpu_lanes`), each once.
    handed: Vec<u32>,
    since: u32,
}

/// The tree's lanes as the GPU takes them (`Store::gpu_lanes`): both as
/// they live, the room either has before it grows, the store's revision,
/// and the blocks written since the lanes were last handed, at `since`, by
/// their first slots: a copy of them at `since` with those blocks taken
/// again is the lanes at `revision`.
pub struct GpuLanes<'a> {
    pub heap: &'a [u32],
    pub data: &'a [u32],
    pub room: usize,
    pub revision: u32,
    pub since: u32,
    pub written: &'a [u32],
}

impl Default for Store {
    fn default() -> Store {
        Store {
            tree: Tree::new(EMPTY.word()),
            ext: Pools::default(),
            revision: 0,
            handed: Vec::new(),
            since: 0,
        }
    }
}

pub(super) type Tree = SparseTree<u32, 3>;

/// A node met in a descent: its zoom, its index and cube in the asker's frame,
/// its value if it is a leaf; whether to go under it.
type Enter<'a> = dyn FnMut(Zoom, [i32; 3], &Aabb, Option<Leaf>) -> bool + 'a;

impl Store {
    /// The tree's lanes, read where they live.
    pub fn lanes(&self) -> Lanes<'_> {
        Lanes(&self.tree)
    }

    /// What is placed as of now, which every write moves.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The tree's lanes as the GPU takes them (`GpuLanes`): the blocks
    /// written since the last hand, each once, and the revision it was at.
    pub fn gpu_lanes(&mut self) -> GpuLanes<'_> {
        self.tree.take_written(&mut self.handed);
        self.handed.sort_unstable();
        self.handed.dedup();
        let since = core::mem::replace(&mut self.since, self.revision);
        GpuLanes {
            heap: self.tree.heap(),
            data: self.tree.data(),
            room: self.tree.room(),
            revision: self.revision,
            since,
            written: &self.handed,
        }
    }

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        for k in 0..4 {
            self.walk(self.cursor((0, root(k)), 0), 0, root(k), about, enter);
        }
    }

    /// `descend` under the node `at` of `zoom` that holds `held`, by the step
    /// every descent of the tree takes (`overwrite::child`).
    fn walk(&self, held: Held, zoom: Zoom, at: [i32; 3], about: f64, enter: &mut Enter) {
        if !held.holds() {
            return;
        }
        let leaf = (!held.folded()).then(|| Leaf::of(self.tree.data()[held.slot() as usize]));
        let here = node_near(zoom, at, about);
        if !enter(zoom, here, &cube(zoom, here), leaf) || !held.folded() {
            return;
        }
        for k in 0..8 {
            let child = wrap_cell(zoom + 1, child_of(at, k));
            self.walk(self.child(&held, (zoom + 1, child), zoom + 1), zoom + 1, child, about, enter);
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

    /// `entries_in`, from the nodes `from` names, each with what the tree holds
    /// there (`overwrite::Held`, a descent's), none under another and
    /// `query` within them: each walked from its slot, so what is found and
    /// the order it is found in are the walk's from the roots, taken in the
    /// tree's own order (`tree_key`).
    pub fn entries_under(&self, from: &[(Node<3>, Held)], query: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        let about = about(query);
        for &((zoom, at), held) in from {
            if !held.holds() {
                continue;
            }
            let z = held.zoom();
            let at = wrap_cell(z, at.map(|c| c >> (zoom - z)));
            self.walk(held, z, at, about, &mut |zoom, at, whole, leaf| {
                if whole.meet(*query).is_none() {
                    return false;
                }
                if let Some(v) = leaf {
                    visit(&Entry::of(zoom, at, v));
                }
                true
            });
        }
    }

    /// Every leaf meeting the cube at `zoom` that holds `held`, X in the
    /// cube's frame: walked from the slot the cursor names, the node's own or
    /// the leaf over it.
    fn each_in(&self, zoom: Zoom, at: [i32; 3], held: Held, visit: &mut dyn FnMut(&Entry)) {
        if !held.holds() {
            return;
        }
        let z = held.zoom();
        self.walk(held, z, at.map(|c| c >> (zoom - z)), about(&cube(zoom, at)), &mut |zoom, at, _, leaf| {
            if let Some(v) = leaf {
                visit(&Entry::of(zoom, at, v));
            }
            true
        });
    }

    /// The value placed at exactly the node `at` of `zoom`, if one is, read
    /// off the cursor a descent holds there (`Query::cursor`, `Query::child`).
    pub(crate) fn placed(&self, zoom: Zoom, at: [i32; 3], held: Held) -> Option<Entry> {
        (held.holds() && !held.folded() && held.zoom() == zoom).then(|| Entry::of(zoom, at, Leaf::of(self.tree.data()[held.slot() as usize])))
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
                self.revision = self.revision.wrapping_add(1);
            }
        }
        changed
    }

    /// Frees the records of the leaves the node at `zoom` replaces: a leaf
    /// at it or under it. One over it with a record is never split.
    fn release(&mut self, zoom: Zoom, at: [i32; 3]) {
        let mut freed = Vec::new();
        let held = self.cursor((zoom, at), zoom);
        if held.holds() && !held.folded() {
            let z = held.zoom();
            xpute_kit::ensure!(z == zoom || Leaf::of(self.tree.data()[held.slot() as usize]).ext == NONE, ENOTRECOVERABLE, z);
        }
        self.each_in(zoom, at, held, &mut |e| {
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
        self.revision = self.revision.wrapping_add(1);
        held
    }
}

impl Entry {
    fn of(zoom: Zoom, at: [i32; 3], leaf: Leaf) -> Entry {
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
        self.store.each_in(zoom, at, self.store.cursor(self.node, zoom), visit)
    }

    /// The leaf placed at exactly the node, if one is.
    pub fn placed(&self) -> Option<Entry> {
        let (zoom, at) = self.node;
        self.store.placed(zoom, at, self.store.cursor(self.node, zoom))
    }
}

impl Query for Store {
    /// What a cube holds, its parent's handed down (pxl-spec-core `overwrite`);
    /// a node over the ground's two axes is read under it.
    type Cursor = Held;

    fn cursor<const D: usize>(&self, (zoom, at): Node<D>, _: Zoom) -> Held {
        match D {
            3 => overwrite::cursor(&self.lanes(), overwrite::PLACED, zoom, [at[0], at[1], at[2]]),
            _ => Held::branch(0, zoom),
        }
    }

    fn child<const D: usize>(&self, parent: &Held, (zoom, at): Node<D>, _: Zoom) -> Held {
        match D {
            3 => overwrite::child(&self.lanes(), *parent, zoom, [at[0], at[1], at[2]]),
            _ => *parent,
        }
    }

    /// All resident, so every answer is exact. The cubes of the node's own
    /// zoom that hold anything, joined: a cube is its own, whole where it
    /// holds, and a column (a node over the ground's two axes) the cubes of
    /// it that hold (`column`). Its readers tell apart no finer than the
    /// node (pxl-kernel view/observation.rs `cubes`, `order_of_tile`).
    fn bound<const D: usize>(&self, held: &Held, node: Node<D>, _: Zoom) -> Answer<Option<Aabb>> {
        Answer::Exact(match D {
            3 => held.holds().then(|| region(node)),
            _ => self.column(&mut Walk::default(), (node.0, [node.1[0], node.1[1]])),
        })
    }
}

/// What a walk over columns carries down the tree from one column to the
/// next (`Store::column`): for each zoom it stepped through, the column it
/// stood in there, where that zoom's run of `held` ends and the z the
/// leaves over it span; each run, the branches the tree holds in that
/// column at that zoom, by index (`overwrite::path`) with what each holds.
/// A walk to a column beside the last steps down only from where their
/// columns part. Good while the store is unchanged.
#[derive(Default)]
pub struct Walk {
    at: Vec<Stood>,
    held: Vec<([i32; 3], Held)>,
}

#[derive(Clone, Copy)]
struct Stood {
    column: [i32; 2],
    end: usize,
    leaves: Option<[f64; 2]>,
}

impl Walk {
    /// Where zoom `k`'s run of `held` begins: where the zoom over it ends.
    fn start(&self, k: usize) -> usize {
        k.checked_sub(1).map_or(0, |o| self.at[o].end)
    }
}

impl Store {
    /// The column `node` of the ground's two axes, X in the asker's frame,
    /// as far along z as what is placed in it reaches: the cubes of its zoom
    /// that hold and the leaves over them, none where nothing is placed in
    /// it. Stepped down from where `walk` last went, a zoom at a time,
    /// through the branches in its column alone.
    pub fn column(&self, walk: &mut Walk, (zoom, xy): Node<2>) -> Option<Aabb> {
        let p = path(zoom, [xy[0], xy[1], 0]);
        let column = |k: usize| [p[0] >> (zoom as usize - k), p[1] >> (zoom as usize - k)];
        let kept = walk.at.iter().take(zoom as usize + 1).enumerate().take_while(|&(k, s)| s.column == column(k)).count();
        walk.at.truncate(kept);
        walk.held.truncate(walk.at.last().map_or(0, |s| s.end));
        let span = |z: usize, at: [i32; 3]| [f64::from(at[2]) * units(z as Zoom), f64::from(at[2] + 1) * units(z as Zoom)];
        let join = |a: Option<[f64; 2]>, b: [f64; 2]| Some(a.map_or(b, |a| [a[0].min(b[0]), a[1].max(b[1])]));
        while walk.at.len() <= zoom as usize {
            let k = walk.at.len();
            let c = column(k);
            let mut leaves = k.checked_sub(1).and_then(|o| walk.at[o].leaves);
            let mut met = |held: &mut Vec<([i32; 3], Held)>, at: [i32; 3], h: Held| {
                if h.folded() {
                    held.push((at, h));
                } else if h.holds() {
                    leaves = join(leaves, span(k, at));
                }
            };
            match k.checked_sub(1) {
                None => {
                    for r in (0..4).map(root).filter(|r| [r[0], r[1]] == c) {
                        met(&mut walk.held, r, self.cursor((0, r), 0));
                    }
                }
                Some(o) => {
                    for i in walk.start(o)..walk.at[o].end {
                        let (at, h) = walk.held[i];
                        for child in [0, 1].map(|z| [c[0], c[1], 2 * at[2] + z]) {
                            met(&mut walk.held, child, self.child(&h, (k as Zoom, child), k as Zoom));
                        }
                    }
                }
            }
            walk.at.push(Stood {
                column: c,
                end: walk.held.len(),
                leaves,
            });
        }
        let k = zoom as usize;
        let z = walk.held[walk.start(k)..walk.at[k].end].iter().fold(walk.at[k].leaves, |z, &(at, _)| join(z, span(k, at)))?;
        let whole = region((zoom, xy));
        let placed = Aabb {
            min: [whole.min[0], whole.min[1], z[0]],
            max: [whole.max[0], whole.max[1], z[1]],
        };
        placed.meet(whole)
    }
}

impl Store {
    /// What is placed in the node, read where it stands: all resident.
    pub fn read(&self, node: Node<3>) -> Cells<'_> {
        Cells { store: self, node }
    }
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
