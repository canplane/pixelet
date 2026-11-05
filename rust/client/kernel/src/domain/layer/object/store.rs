// pxl-kernel/domain/layer/object/store.rs

//! Placed objects, kept as their one record: each leaf's catalog value at its
//! node and the handle of what it carries beyond it (`ext.rs`), in one sparse
//! tree over the world (`space/hierarchy/sparse_tree.rs`), whose root is the
//! world, a node of zoom 0. A write arrives as `<key, id, ext>`
//! (`morton::key_of`). What a node at any zoom holds is the pyramid's
//! (`pyramid.rs`), each branch's slot its children folded, folded again by
//! each write up its path, read through the layer's query (`Query::cursor`,
//! `Query::child`, `Store::value`). A leaf that carries a record is never
//! joined, its handle its own, and is never split: a placement takes an
//! empty cell and an erase takes the whole leaf. A node at `SHARD_ZOOM`
//! whose shard is not read holds what its source says it folds to, its
//! children empty (`set_fold`), until the shard's leaves take its place.

use xpute_guest::global::Global;

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
use pxl_engine::overwrite::{self, path, NodeSlot};
use pxl_render::gpu::api;
use pxl_spec::object::ext::class_of;
use pxl_spec::object::volume::fold;

/// A leaf as an asker meets it: its cube, X in the asker's frame, and the
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

    /// What it is, as `Asset::of` reads what it holds.
    pub fn asset(&self) -> Asset {
        Asset::of(self.state)
    }
}

/// The placed objects, one store, the layer's own.
static OBJECT: Global<Store> = Global::new();

pub fn object() -> &'static mut Store {
    OBJECT.get(Store::default)
}

/// Applies every write (`Store::write`); whether any changed the store.
pub(crate) fn apply(writes: impl IntoIterator<Item = Write>) -> bool {
    let regions = object().write(writes);
    if !regions.is_empty() {
        note_written(&regions);
    }
    !regions.is_empty()
}

static WRITTEN: Global<Vec<Aabb>> = Global::new();

/// Where the objects changed, kept for the turn to tell what draws them and
/// what is worked out from them (`take_written`).
pub(super) fn note_written(regions: &[Aabb]) {
    WRITTEN.get(Vec::new).extend_from_slice(regions);
    request_world_tick();
}

/// The regions written since the turn last took them, into `out`.
pub fn take_written(out: &mut Vec<Aabb>) {
    out.clear();
    core::mem::swap(out, WRITTEN.get(Vec::new));
}

pub const VOID: AssetId = cube::sentinel::void as u32;
pub const AIR: AssetId = cube::object::air as u32;
const _: () = assert!(VOID == 0);

/// What a tree's leaf holds: the catalog value, and its record's handle.
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
            id: overwrite::low(word) as u16,
            ext: overwrite::high(word) as u16,
        }
    }

    /// Its word in the tree (pxl-engine `overwrite::leaf_word`).
    pub(super) const fn word(self) -> u32 {
        overwrite::leaf_word(self.id as u32, self.ext as u32)
    }
}

/// Levels under the tree's root of a node at `zoom`.
const fn depth(zoom: Zoom) -> u32 {
    zoom + 1
}

/// The tree's columns as pxl-engine `overwrite` reads them, the CPU as a GPU.
pub struct Columns<'a>(&'a Tree);

impl overwrite::Columns for Columns<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        self.0.first_child()[slot as usize]
    }
    fn data(&self, slot: u32) -> u32 {
        self.0.data()[slot as usize]
    }
}

/// Where the node `at` of `zoom` comes in the tree's own order, the order a
/// walk from the roots enters nodes in (`descend`): its slot a zoom
/// (`child_idx`) from the roots down, so of two nodes neither under the other
/// the one entered first is the lesser.
pub(crate) fn tree_key(zoom: Zoom, at: [i32; 3]) -> u128 {
    let p = path(zoom, at);
    let mut key = 0u128;
    for l in 0..=zoom {
        key = key << 3 | child_idx(p.map(|v| v >> (zoom - l))) as u128;
    }
    key << (3 * (OBJECT_FINEST - zoom))
}

/// One leaf written: `<key, id, ext>`, its node, its catalog value and what
/// it carries. VOID erases.
#[derive(Clone, Debug)]
pub struct Write {
    pub at: u128,
    pub id: AssetId,
    pub ext: Option<Ext>,
}

impl Write {
    /// `id` at the cube `at` names.
    pub fn at(at: Octant, id: AssetId) -> Write {
        Write {
            at: at.id(),
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
    /// copy of its columns, the GPU's, knows to take them again.
    revision: u32,
    /// The child groups the tree wrote between the revision the columns were last
    /// handed at (`since`) and that hand (`gpu_columns`), each once.
    written_groups: Vec<u32>,
    since: u32,
}

/// The tree's columns as the GPU takes them (`Store::gpu_columns`): both as
/// they live, the room either has before it grows, the store's revision,
/// and the child groups written since the columns were last handed, at `since`, by
/// their first slots: a copy of them at `since` with those child groups taken
/// again is the columns at `revision`.
pub struct GpuColumns<'a> {
    pub first_child: &'a [u32],
    pub data: &'a [u32],
    pub room: usize,
    pub revision: u32,
    pub since: u32,
    pub written_groups: &'a [u32],
}

impl Default for Store {
    fn default() -> Store {
        Store {
            tree: Tree::new(EMPTY.word()),
            ext: Pools::default(),
            revision: 0,
            written_groups: Vec::new(),
            since: 0,
        }
    }
}

pub(super) type Tree = SparseTree<u32, 3>;

/// A node met in a descent: its zoom, its index and cube in the asker's frame,
/// its value if it is a leaf; whether to go under it.
type Enter<'a> = dyn FnMut(Zoom, [i32; 3], &Aabb, Option<Leaf>) -> bool + 'a;

impl Store {
    /// What is placed as of now, which every write moves.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Whether the host holds the tree's lanes, which the renderer hands
    /// where they lie (pxl-render gpu/api.rs `in_flight`): nothing writes the
    /// tree until it gives them back (repository.rs `Change`).
    pub(crate) fn is_in_flight(&self) -> bool {
        api::in_flight(self.tree.first_child()) || api::in_flight(self.tree.data())
    }

    /// The tree's columns as the GPU takes them (`GpuColumns`): the child groups
    /// written since the last hand, each once, and the revision it was at.
    pub fn gpu_columns(&mut self) -> GpuColumns<'_> {
        self.tree.take_written(&mut self.written_groups);
        self.written_groups.sort_unstable();
        self.written_groups.dedup();
        let since = core::mem::replace(&mut self.since, self.revision);
        GpuColumns {
            first_child: self.tree.first_child(),
            data: self.tree.data(),
            room: self.tree.room(),
            revision: self.revision,
            since,
            written_groups: &self.written_groups,
        }
    }

    /// Every node anything is under, from the roots down, in `about`'s
    /// frame, a node entered where `enter` says so: its zoom, its index and
    /// cube there, and its value if it is a leaf.
    fn descend(&self, about: f64, enter: &mut Enter) {
        self.walk(self.cursor((0, [0; 3]), 0), 0, [0; 3], about, enter);
    }

    /// `descend` under the node `at` of `zoom` that holds `slot`, by the step
    /// every descent of the tree takes (`overwrite::child`).
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
            let child = wrap_cell(zoom + 1, overwrite::child_at(zoom, at, k));
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
    /// Every leaf, as the columns name it.
    pub fn each(&self, visit: &mut dyn FnMut(&Entry)) {
        self.descend(f64::NAN, &mut |zoom, at, _, leaf| {
            if let Some(v) = leaf {
                visit(&Entry::of(zoom, at, v));
            }
            true
        });
    }

    /// Every leaf meeting the cube at `zoom` that holds `slot`, X in the
    /// cube's frame: walked from the slot the cursor names, the node's own or
    /// the leaf over it.
    fn each_in(&self, zoom: Zoom, at: [i32; 3], slot: NodeSlot, visit: &mut dyn FnMut(&Entry)) {
        if !slot.holds() {
            return;
        }
        let z = slot.zoom();
        self.walk(
            slot,
            z,
            overwrite::ancestor_at(zoom, at, zoom - z),
            about(&cube(zoom, at)),
            &mut |zoom, at, _, leaf| {
                if let Some(v) = leaf {
                    visit(&Entry::of(zoom, at, v));
                }
                true
            },
        );
    }

    /// Applies every write; the cubes that changed. A write whose record its
    /// pool has no room for is left out, as if never sent. A write with a
    /// record changes its cube whatever its word: the record it replaces is
    /// freed first, so its slot is the one reused, and new text under the old
    /// handle leave the word as it was.
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
            // A record is the shape its id's class names, or none.
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

    /// Frees the records of the leaves the node at `zoom` replaces: a leaf
    /// at it or under it. One over it with a record is never split.
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

    /// The shard `at` held as `word`, what its leaves fold to
    /// (pxl-engine `overwrite::fold_word`), they not here: a branch whose
    /// children hold nothing, so it is drawn as its fold from where its zoom
    /// or a coarser one is a cell, and nothing finer is met in it. Its cube.
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

    /// The shard `at` emptied where `set_fold` held it, no leaf under it; its
    /// cube where it changed.
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

    /// The word the shard `at` holds in the tree: what its leaves fold to; none
    /// where nothing is under it.
    #[cfg(test)]
    pub(crate) fn word_at(&self, [x, y]: [i32; 2]) -> Option<u32> {
        let at = [x, y, 0];
        let slot = self.cursor((SHARD_ZOOM, at), SHARD_ZOOM);
        (slot.holds() && slot.zoom() == SHARD_ZOOM).then(|| self.tree.data()[slot.slot() as usize])
    }

    #[cfg(test)]
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
    /// What a cube holds, its parent's handed down (pxl-engine `overwrite`);
    /// a node over the ground's two axes is read under it.
    type Cursor = NodeSlot;

    fn cursor<const D: usize>(&self, (zoom, at): Node<D>, _: Zoom) -> NodeSlot {
        match D {
            3 => overwrite::cursor(
                &self.columns(),
                overwrite::OBJECT_ROOT,
                zoom,
                [at[0], at[1], at[2]],
            ),
            _ => NodeSlot::branch(0, zoom),
        }
    }

    fn child<const D: usize>(&self, parent: &NodeSlot, (zoom, at): Node<D>, _: Zoom) -> NodeSlot {
        match D {
            3 => overwrite::child(
                &self.columns(),
                *parent,
                zoom,
                path(zoom, [at[0], at[1], at[2]]),
            ),
            _ => *parent,
        }
    }

    /// All resident, so every answer is exact. The cubes of the node's own
    /// zoom that hold anything, joined: a cube is its own, whole where it
    /// holds, and a column (a node over the ground's two axes) the cubes of
    /// it that hold (`column`). Its readers tell apart no finer than the
    /// node (pxl-kernel domain/view/observation.rs `bricks`, `order_of_tile`).
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

/// What a walk over columns carries down the tree from one column to the
/// next (`Store::column`): for each zoom it stepped through, the column it
/// stood in there, where that zoom's run of `slots` ends and the z the
/// leaves over it span; each run, the branches the tree holds in that
/// column at that zoom, by index (`overwrite::path`) with what each holds.
/// A walk to a column beside the last steps down only from where their
/// columns part. Good while the store is unchanged.
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

/// What the store holds, read: its columns, the leaves in a box or a node,
/// what a leaf carries, and a column's reach. Read by the world's query alone
/// (domain/query.rs), as `Query` is.
pub(in crate::domain) trait ObjectQuery {
    /// The tree's columns, read where they live.
    fn columns(&self) -> Columns<'_>;

    /// Every leaf whose cube meets `query`, X in `query`'s frame.
    fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry));

    /// `entries_in`, from the nodes `from` names, each with what the tree holds
    /// there (`overwrite::NodeSlot`, a descent's), none under another and
    /// `query` within them: each walked from its slot, so what is found and
    /// the order it is found in are the walk's from the roots, taken in the
    /// tree's own order (`tree_key`).
    fn entries_under(
        &self,
        from: &[(Node<3>, NodeSlot)],
        query: &Aabb,
        visit: &mut dyn FnMut(&Entry),
    );

    /// The value placed at exactly the node `at` of `zoom`, if one is, read
    /// off the cursor a descent holds there (`Query::cursor`, `Query::child`).
    fn entry_at(&self, zoom: Zoom, at: [i32; 3], slot: NodeSlot) -> Option<Entry>;

    /// What a leaf carries beyond its id, if anything.
    fn ext(&self, e: &Entry) -> Option<&Ext>;

    /// A marker's text, as put up; none for anything else.
    fn text(&self, e: &Entry) -> Option<&str>;

    /// The column `node` of the ground's two axes, X in the asker's frame,
    /// as far along z as what is placed in it reaches: the cubes of its zoom
    /// that hold and the leaves over them, none where nothing is placed in
    /// it. Stepped down from where `walk` last went, a zoom at a time,
    /// through the branches in its column alone.
    fn column(&self, walk: &mut Walk, node: Node<2>) -> Option<Aabb>;

    /// What is placed in the node, read where it stands: all resident.
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
            let at = wrap_cell(z, overwrite::ancestor_at(zoom, at, zoom - z));
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
                        let z = 2 * (at[2] + overwrite::z_start(k as Zoom - 1))
                            - overwrite::z_start(k as Zoom);
                        let y = c[1] - overwrite::y_start(k as Zoom);
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
