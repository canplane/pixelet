// pxl-core/domain/layer/object/store.rs

//! The object layer's store: placed objects, kept as their one record. Each
//! leaf holds its catalog value at its node and the handle of the record it
//! carries beyond that value (ext.rs). The leaves live in one sparse tree
//! over the world (pxl-spec space/hierarchy/sparse_tree.rs), whose root is
//! the world, a node of zoom 0. A write arrives as `<key, id, ext>`, keyed by
//! its node's Morton key (pxl-spec `morton::key_of`).
//!
//! Each branch's slot holds its children folded into one word (pxl-spec
//! object/volume.rs `fold`), and every write folds again up its path, so a
//! node at any zoom can be read through the layer's query (`Query::cursor`,
//! `Query::child`).
//!
//! A leaf that carries a record is never joined with its siblings, since its
//! handle is its own, and it is never split: a placement takes an empty
//! cell, and an erase takes the whole leaf.
//!
//! A node at `SHARD_ZOOM` whose shard is not read yet holds the fold its
//! source gives, with empty children (`set_fold`), until the shard's leaves
//! take its place.

use xpute_core::global::Global;

use crate::domain::layer::object::asset::Asset;
use pxl_spec::asset::{cube, AssetId};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::{node_near, wrap_cell};
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::node_of;
use pxl_spec::space::hierarchy::sparse_tree::SparseTree;
use pxl_spec::space::hierarchy::tree::{child_idx, region, Node};
use pxl_spec::space::layer::octant::Octant;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::object::ext::{Ext, Pools, NONE};
use crate::domain::layer::{Answer, Query};
use crate::sys::sched::edge::request_world_tick;
use pxl_engine::object::{self, path, NodeSlot};
use pxl_render::gpu::api;
use pxl_spec::object::ext::class_of;
use pxl_spec::object::volume::fold;

/// A leaf as an asker meets it: its cube, with its X index taken in the
/// asker's frame since X wraps around the world, its catalog value, and the
/// handle of its record in its class's pool.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    pub zoom: Zoom,
    pub at: [i32; 3],
    pub state: AssetId,
    pub ext: u16,
}

impl Entry {
    pub fn id(&self) -> Octant {
        Octant::new(self.zoom, self.at)
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

    pub fn asset(&self) -> Asset {
        Asset::of(self.state)
    }
}

/// The layer's one store of placed objects.
static OBJECT: Global<Store> = Global::new();

pub fn object() -> &'static Store {
    object_mut()
}

pub fn object_mut() -> &'static mut Store {
    OBJECT.get(Store::default)
}

/// Applies every write (`Store::write`) and notes where the store changed.
/// Returns whether any write changed it.
pub(crate) fn apply(writes: impl IntoIterator<Item = Write>) -> bool {
    let regions = object_mut().write(writes);
    if !regions.is_empty() {
        note_written(&regions);
    }
    !regions.is_empty()
}

static WRITTEN: Global<Vec<Aabb>> = Global::new();

/// Keeps the regions where the objects changed, for the turn to pass on to
/// what draws the objects and what is derived from them (`take_written`).
pub(super) fn note_written(regions: &[Aabb]) {
    WRITTEN.get(Vec::new).extend_from_slice(regions);
    request_world_tick();
}

/// Moves into `out` the regions written since the turn last took them.
pub fn take_written(out: &mut Vec<Aabb>) {
    out.clear();
    core::mem::swap(out, WRITTEN.get(Vec::new));
}

pub const VOID: AssetId = cube::sentinel::void as u32;
pub const AIR: AssetId = cube::object::air as u32;
const _: () = assert!(VOID == 0);

/// What a leaf of the tree holds: its catalog value and its record's handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaf {
    pub id: u16,
    pub ext: u16,
}

pub(super) const EMPTY: Leaf = Leaf {
    id: VOID as u16,
    ext: NONE,
};

impl Leaf {
    pub(super) fn of(word: u32) -> Leaf {
        Leaf {
            id: object::low(word) as u16,
            ext: object::high(word) as u16,
        }
    }

    /// The leaf's word in the tree, laid out as pxl-engine `object::leaf_word`
    /// lays it.
    pub(super) const fn word(self) -> u32 {
        object::leaf_word(self.id as u32, self.ext as u32)
    }
}

/// The levels of the tree from its root down to a node at `zoom`, both
/// counted.
const fn depth(zoom: Zoom) -> u32 {
    zoom + 1
}

/// The tree's columns as pxl-engine `object` reads them, so that the CPU
/// walks the tree exactly as the GPU does.
pub struct Columns<'a>(&'a Tree);

impl object::Columns for Columns<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        self.0.first_child()[slot as usize]
    }
    fn data(&self, slot: u32) -> u32 {
        self.0.data()[slot as usize]
    }
}

/// The position of node `at` of `zoom` in the tree's own order, the order in
/// which a walk from the root enters nodes (`descend`). The key is the node's
/// child index at each zoom (`child_idx`) from the root down, so of two
/// nodes where neither is under the other, the one entered first has the
/// smaller key.
pub(crate) fn tree_key(zoom: Zoom, at: [i32; 3]) -> u128 {
    let p = path(zoom, at);
    let mut key = 0u128;
    for l in 0..=zoom {
        key = key << 3 | child_idx(p.map(|v| v >> (zoom - l))) as u128;
    }
    key << (3 * (OBJECT_FINEST - zoom))
}

/// One leaf written, as `<key, id, ext>`: its node's key, its catalog value
/// and the record it carries. An id of `VOID` erases the leaf.
#[derive(Clone, Debug)]
pub struct Write {
    pub at: u128,
    pub id: AssetId,
    pub ext: Option<Ext>,
}

impl Write {
    /// A write of `id` at cube `at`, with no record.
    pub fn at(at: Octant, id: AssetId) -> Write {
        Write {
            at: at.id(),
            id,
            ext: None,
        }
    }
}

/// The placed objects: one tree of leaves in which each branch holds its
/// children folded, one word for each slot.
pub struct Store {
    tree: Tree,
    ext: Pools,
    /// A counter that advances with every write that changes the tree, so a
    /// reader holding a copy of the columns, such as the GPU, knows to take
    /// them again.
    revision: u32,
}

/// The tree's columns as the GPU takes them (`Store::gpu_columns`), in
/// place, at `revision`. `cap` is the room each column has before it grows.
pub struct GpuColumns<'a> {
    pub first_child: &'a [u32],
    pub data: &'a [u32],
    pub cap: usize,
    pub revision: u32,
}

impl Default for Store {
    fn default() -> Store {
        Store {
            tree: Tree::new(EMPTY.word()),
            ext: Pools::default(),
            revision: 0,
        }
    }
}

pub(super) type Tree = SparseTree<u32, 3>;

/// The visitor a descent calls on each node it meets, with the node's zoom,
/// its index and cube in the asker's frame, and its value if it is a leaf.
/// It returns whether to go under the node.
type Enter<'a> = dyn FnMut(Zoom, [i32; 3], &Aabb, Option<Leaf>) -> bool + 'a;

impl Store {
    /// The store's revision, which every write that changes the store
    /// advances.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Whether the runtime holds the tree's columns, which the renderer lends
    /// it in place for the GPU to read (pxl-render gpu/api.rs `in_flight`).
    /// Nothing may write the tree until the runtime gives them back
    /// (repository.rs `Change`).
    pub(crate) fn is_in_flight(&self) -> bool {
        api::in_flight(self.tree.first_child()) || api::in_flight(self.tree.data())
    }

    /// The tree's columns as the GPU takes them (`GpuColumns`).
    pub fn gpu_columns(&self) -> GpuColumns<'_> {
        GpuColumns {
            first_child: self.tree.first_child(),
            data: self.tree.data(),
            cap: self.tree.cap(),
            revision: self.revision,
        }
    }

    /// Moves into `into` the child groups the tree wrote since they were last
    /// taken, by their first slots, each once. The one who takes them keeps
    /// what it took (domain/scene/cast.rs `objects`).
    pub fn take_written(&mut self, into: &mut Vec<u32>) {
        self.tree.take_written(into);
        into.sort_unstable();
        into.dedup();
    }

    /// Walks every node that holds anything, from the root down, with indices
    /// in the frame around X = `about`. It goes under a node only where
    /// `enter` says so.
    fn descend(&self, about: f64, enter: &mut Enter) {
        self.walk(self.cursor((0, [0; 3]), 0), 0, [0; 3], about, enter);
    }

    /// `descend` from node `at` of `zoom`, whose slot is `slot`, taking the
    /// same step every descent of the tree takes (`object::child`).
    fn walk(&self, slot: NodeSlot, zoom: Zoom, at: [i32; 3], about: f64, enter: &mut Enter) {
        if !slot.holds() {
            return;
        }
        let leaf = (!slot.is_folded()).then(|| Leaf::of(self.tree.data()[slot.slot() as usize]));
        let here = node_near(zoom, at, about);
        if !enter(zoom, here, &cube(zoom, here), leaf) || !slot.is_folded() {
            return;
        }
        for k in 0..8 {
            let child = wrap_cell(zoom + 1, object::child_at(zoom, at, k));
            self.walk(
                self.child(&slot, (zoom + 1, child), zoom + 1),
                zoom + 1,
                child,
                about,
                enter,
            );
        }
    }

    #[cfg(test)]
    /// Calls `visit` on every leaf, as the columns name it.
    pub fn each(&self, visit: &mut dyn FnMut(&Entry)) {
        self.descend(f64::NAN, &mut |zoom, at, _, leaf| {
            if let Some(v) = leaf {
                visit(&Entry::of(zoom, at, v));
            }
            true
        });
    }

    /// Calls `visit` on every leaf that meets cube `at` of `zoom`, with X in
    /// the cube's frame. `slot` is the cursor at that cube, either the cube's
    /// own slot or the leaf over it, and the walk starts there.
    fn each_in(&self, zoom: Zoom, at: [i32; 3], slot: NodeSlot, visit: &mut dyn FnMut(&Entry)) {
        if !slot.holds() {
            return;
        }
        let z = slot.zoom();
        self.walk(
            slot,
            z,
            object::ancestor_at(zoom, at, zoom - z),
            about(&cube(zoom, at)),
            &mut |zoom, at, _, leaf| {
                if let Some(v) = leaf {
                    visit(&Entry::of(zoom, at, v));
                }
                true
            },
        );
    }

    /// Applies every write and returns the cubes that changed. A write whose
    /// record finds no room in its pool is skipped, as if it was never sent.
    /// A write with a record changes its cube even when the word stays the
    /// same: the record it replaces is freed first, so the new record reuses
    /// that slot and handle, and new text under the old handle leaves the
    /// word as it was.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> Vec<Aabb> {
        let mut changed = Vec::new();
        for Write { at: k, id, ext } in writes {
            let (zoom, at) = node_of::<3>(k);
            xpute_kit::ensure!(
                (CUBE_ZOOM..=OBJECT_FINEST).contains(&zoom),
                ENOTRECOVERABLE,
                zoom
            );
            let id16 = u16::try_from(id).or_bug(Errno::EOVERFLOW);
            // A record must be of the class its id names, or absent.
            xpute_kit::ensure!(
                ext.as_ref().map(Ext::class) == ext.as_ref().and(class_of(id)),
                EINVAL,
                id
            );
            self.release(zoom, at);
            let handle = match ext {
                Some(ext) => match self.ext.keep(ext) {
                    Some(h) => h,
                    None => continue,
                },
                None => NONE,
            };
            let val = Leaf {
                id: id16,
                ext: handle,
            }
            .word();
            let set = self
                .tree
                .set(depth(zoom), path(zoom, at), val, |tree, child_group, d| {
                    fold(tree, child_group, d - 1)
                });
            if set || handle != NONE {
                changed.push(cube(zoom, at));
                self.revision = self.revision.wrapping_add(1);
            }
        }
        changed
    }

    /// Frees the records of the leaves that a write to node `at` of `zoom`
    /// replaces: any leaf at it or under it. A leaf over it that carries a
    /// record is never split, and finding one stops the core.
    fn release(&mut self, zoom: Zoom, at: [i32; 3]) {
        let mut freed = Vec::new();
        let slot = self.cursor((zoom, at), zoom);
        if slot.holds() && !slot.is_folded() {
            let z = slot.zoom();
            xpute_kit::ensure!(
                z == zoom || Leaf::of(self.tree.data()[slot.slot() as usize]).ext == NONE,
                ENOTRECOVERABLE,
                z
            );
        }
        self.each_in(zoom, at, slot, &mut |e| {
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

    /// Holds shard `at` as `word`, the fold of its leaves (pxl-engine
    /// `object::fold_word`), while the leaves themselves are not here. The
    /// shard becomes a branch whose children hold nothing, so it is drawn as
    /// its fold wherever its zoom or a coarser one is a cell, and nothing
    /// finer is met inside it. Returns the shard's cube.
    pub(crate) fn set_fold(&mut self, [x, y]: [i32; 2], word: u32) -> Aabb {
        let at = [x, y, 0];
        self.tree.set_branch(
            depth(SHARD_ZOOM),
            path(SHARD_ZOOM, at),
            word,
            EMPTY.word(),
            |tree, child_group, d| fold(tree, child_group, d - 1),
        );
        self.revision = self.revision.wrapping_add(1);
        cube(SHARD_ZOOM, at)
    }

    /// Empties shard `at` where `set_fold` held it and no leaf is under it.
    /// Returns the shard's cube when it changed.
    pub(crate) fn clear_fold(&mut self, [x, y]: [i32; 2]) -> Option<Aabb> {
        let at = [x, y, 0];
        let slot = self.cursor((SHARD_ZOOM, at), SHARD_ZOOM);
        if !slot.is_folded() || slot.zoom() != SHARD_ZOOM {
            return None;
        }
        let mut leaf = false;
        self.each_in(SHARD_ZOOM, at, slot, &mut |_| leaf = true);
        if leaf {
            return None;
        }
        self.tree.set(
            depth(SHARD_ZOOM),
            path(SHARD_ZOOM, at),
            EMPTY.word(),
            |tree, child_group, d| fold(tree, child_group, d - 1),
        );
        self.revision = self.revision.wrapping_add(1);
        Some(cube(SHARD_ZOOM, at))
    }

    /// The word shard `at` holds in the tree, the fold of its leaves; none
    /// where nothing is under it.
    #[cfg(test)]
    pub(crate) fn word_at(&self, [x, y]: [i32; 2]) -> Option<u32> {
        let at = [x, y, 0];
        let slot = self.cursor((SHARD_ZOOM, at), SHARD_ZOOM);
        (slot.holds() && slot.zoom() == SHARD_ZOOM).then(|| self.tree.data()[slot.slot() as usize])
    }

    #[cfg(test)]
    /// Empties the store and returns the cubes that held anything.
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

/// The leaves of one node, as the query reads them. What the node holds
/// folded is read through the cursor instead (`Query::cursor`).
#[derive(Clone, Copy)]
pub struct Cells<'a> {
    store: &'a Store,
    node: Node<3>,
}

impl Cells<'_> {
    #[cfg(test)]
    /// Every leaf inside the node, or the one over it.
    pub fn entries(&self, visit: &mut dyn FnMut(&Entry)) {
        let (zoom, at) = self.node;
        self.store
            .each_in(zoom, at, self.store.cursor(self.node, zoom), visit);
    }

    /// The leaf placed at exactly the node, if one is.
    pub fn entry(&self) -> Option<Entry> {
        let (zoom, at) = self.node;
        self.store
            .entry_at(zoom, at, self.store.cursor(self.node, zoom))
    }
}

impl Query for Store {
    /// The slot that holds a cube, handed down from its parent (pxl-engine
    /// `object`). A node over the ground's two axes, a column, carries no
    /// slot of its own; it is read by walking under it (`column`).
    type Cursor = NodeSlot;

    fn cursor<const D: usize>(&self, (zoom, at): Node<D>, _: Zoom) -> NodeSlot {
        match D {
            3 => object::cursor(&self.columns(), object::ROOT, zoom, [at[0], at[1], at[2]]),
            _ => NodeSlot::branch(0, zoom),
        }
    }

    fn child<const D: usize>(&self, parent: &NodeSlot, (zoom, at): Node<D>, _: Zoom) -> NodeSlot {
        match D {
            3 => object::child(
                &self.columns(),
                *parent,
                zoom,
                path(zoom, [at[0], at[1], at[2]]),
            ),
            _ => *parent,
        }
    }

    /// Every answer is exact, since the whole store is resident. The bound
    /// joins the cubes of the node's own zoom that hold anything: a cube is
    /// bounded by itself, whole where it holds anything, and a column by the
    /// cubes in it that hold anything (`column`). No reader tells positions
    /// apart finer than the node (domain/view/observation.rs `bricks`,
    /// `order_of_tile`).
    fn bound<const D: usize>(
        &self,
        slot: &NodeSlot,
        node: Node<D>,
        _: Zoom,
    ) -> Answer<Option<Aabb>> {
        Answer::Exact(match D {
            3 => slot.holds().then(|| region(node)),
            _ => self.column(&mut Walk::default(), (node.0, [node.1[0], node.1[1]])),
        })
    }
}

/// What a walk over columns carries from one column to the next
/// (`Store::column`). For each zoom it stepped through, `at` holds the
/// column it stood in there, where that zoom's run of `slots` ends, and the
/// z span of the leaves over it. Each run of `slots` lists the branches the
/// tree holds in that column at that zoom, by index (`object::path`), with
/// their slots. A walk to a nearby column steps down only from where the two
/// columns part. It stays valid only while the store is unchanged.
#[derive(Default)]
pub struct Walk {
    at: Vec<Step>,
    slots: Vec<([i32; 3], NodeSlot)>,
}

#[derive(Clone, Copy)]
struct Step {
    column: [i32; 2],
    end: usize,
    leaves: Option<[f64; 2]>,
}

impl Walk {
    /// Where zoom `k`'s run of `slots` begins: where the zoom over it ends.
    fn start(&self, k: usize) -> usize {
        k.checked_sub(1).map_or(0, |o| self.at[o].end)
    }
}

/// The store's reads beyond `Query`. As with `Query`, only the world's query
/// (domain/query.rs) reads it.
pub(in crate::domain) trait ObjectQuery {
    /// The tree's columns, read in place.
    fn columns(&self) -> Columns<'_>;

    /// Calls `visit` on every leaf whose cube meets `query`, with X in
    /// `query`'s frame.
    fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry));

    /// `entries_in`, starting from the nodes in `from` instead of the root.
    /// Each node comes with the slot a descent found for it
    /// (`object::NodeSlot`). No node may be under another, and `query` must
    /// lie within them. Each is walked from its slot, so when `from` is in
    /// the tree's own order (`tree_key`), the leaves found and their order
    /// equal those of a walk from the root.
    fn entries_under(
        &self,
        from: &[(Node<3>, NodeSlot)],
        query: &Aabb,
        visit: &mut dyn FnMut(&Entry),
    );

    /// The leaf placed at exactly node `at` of `zoom`, if any, read from the
    /// cursor a descent holds there (`Query::cursor`, `Query::child`).
    fn entry_at(&self, zoom: Zoom, at: [i32; 3], slot: NodeSlot) -> Option<Entry>;

    /// What a leaf carries beyond its id, if anything.
    fn ext(&self, e: &Entry) -> Option<&Ext>;

    /// A marker's text as it was placed; none for anything else.
    fn text(&self, e: &Entry) -> Option<&str>;

    /// The box of column `node` over the ground's two axes, with X in the
    /// asker's frame, cut along z to what is placed in it: the cubes of its
    /// zoom that hold anything and the leaves over them. None where nothing
    /// is placed in it. The walk steps down from where `walk` last went, one
    /// zoom at a time, through the branches in this column alone.
    fn column(&self, walk: &mut Walk, node: Node<2>) -> Option<Aabb>;

    /// What is placed in `node`, read in place; the whole store is resident.
    fn read(&self, node: Node<3>) -> Cells<'_>;
}

impl ObjectQuery for Store {
    fn columns(&self) -> Columns<'_> {
        Columns(&self.tree)
    }

    fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry)) {
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

    fn entries_under(
        &self,
        from: &[(Node<3>, NodeSlot)],
        query: &Aabb,
        visit: &mut dyn FnMut(&Entry),
    ) {
        let about = about(query);
        for &((zoom, at), slot) in from {
            if !slot.holds() {
                continue;
            }
            let z = slot.zoom();
            let at = wrap_cell(z, object::ancestor_at(zoom, at, zoom - z));
            self.walk(slot, z, at, about, &mut |zoom, at, whole, leaf| {
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

    fn entry_at(&self, zoom: Zoom, at: [i32; 3], slot: NodeSlot) -> Option<Entry> {
        (slot.holds() && !slot.is_folded() && slot.zoom() == zoom)
            .then(|| Entry::of(zoom, at, Leaf::of(self.tree.data()[slot.slot() as usize])))
    }

    fn ext(&self, e: &Entry) -> Option<&Ext> {
        self.ext.get(class_of(e.state)?, e.ext)
    }

    fn text(&self, e: &Entry) -> Option<&str> {
        match self.ext(e)? {
            Ext::MarkerText(t) => Some(t),
        }
    }

    fn column(&self, walk: &mut Walk, (zoom, xy): Node<2>) -> Option<Aabb> {
        let p = path(zoom, [xy[0], xy[1], 0]);
        let column = |k: usize| [p[0] >> (zoom as usize - k), p[1] >> (zoom as usize - k)];
        let shared = walk
            .at
            .iter()
            .take(zoom as usize + 1)
            .enumerate()
            .take_while(|&(k, s)| s.column == column(k))
            .count();
        walk.at.truncate(shared);
        walk.slots.truncate(walk.at.last().map_or(0, |s| s.end));
        let span = |z: usize, at: [i32; 3]| {
            let b = cube(z as Zoom, at);
            [b.min[2], b.max[2]]
        };
        let join = |a: Option<[f64; 2]>, b: [f64; 2]| {
            Some(a.map_or(b, |a| [a[0].min(b[0]), a[1].max(b[1])]))
        };
        while walk.at.len() <= zoom as usize {
            let k = walk.at.len();
            let c = column(k);
            let mut leaves = k.checked_sub(1).and_then(|o| walk.at[o].leaves);
            let mut met = |held: &mut Vec<([i32; 3], NodeSlot)>, at: [i32; 3], h: NodeSlot| {
                if h.is_folded() {
                    held.push((at, h));
                } else if h.holds() {
                    leaves = join(leaves, span(k, at));
                }
            };
            match k.checked_sub(1) {
                None => {
                    met(&mut walk.slots, [0; 3], self.cursor((0, [0; 3]), 0));
                }
                Some(o) => {
                    for i in walk.start(o)..walk.at[o].end {
                        let (at, h) = walk.slots[i];
                        let z = 2 * (at[2] + object::z_start(k as Zoom - 1))
                            - object::z_start(k as Zoom);
                        let y = c[1] - object::y_start(k as Zoom);
                        for child in [z, z + 1].map(|z| [c[0], y, z]) {
                            met(
                                &mut walk.slots,
                                child,
                                self.child(&h, (k as Zoom, child), k as Zoom),
                            );
                        }
                    }
                }
            }
            walk.at.push(Step {
                column: c,
                end: walk.slots.len(),
                leaves,
            });
        }
        let k = zoom as usize;
        let z = walk.slots[walk.start(k)..walk.at[k].end]
            .iter()
            .fold(walk.at[k].leaves, |z, &(at, _)| join(z, span(k, at)))?;
        let whole = region((zoom, xy));
        let placed = Aabb {
            min: [whole.min[0], whole.min[1], z[0]],
            max: [whole.max[0], whole.max[1], z[1]],
        };
        placed.meet(whole)
    }
    fn read(&self, node: Node<3>) -> Cells<'_> {
        Cells { store: self, node }
    }
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
