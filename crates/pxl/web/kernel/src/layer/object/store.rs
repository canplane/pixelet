// pxl-kernel/layer/object/store.rs

//! Placed objects: one octree over the world (`space/sparse_tree.rs`) whose
//! leaves are catalog values at their own zoom. Every node, at every zoom,
//! holds the same three things: a leaf its value, a branch the majority of
//! its eight, the box of everything under it and the finest zoom held there.
//! A write changes the path to its cube and nothing beside it; every query
//! is the same descent at every zoom.

use std::collections::HashMap;
use std::collections::TryReserveError;

use base64::Engine;
pub use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::{CELL, OBJECT_FINEST, U_PER_CELL};
use pxl_spec::bake::fold::majority;
use pxl_spec::cell::object::{decor_id_from_num, decor_num_id, object_value_is_air, object_value_is_decor, object_value_is_voxel, placeable_cube_num_id, placeable_cube_of, PlaceableCube};
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::hierarchy::{child_of, first_descendant_of, parent_of};
use pxl_spec::space::pvf3::ivec::TO_XY;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::LEAF_Z;
use pxl_spec::space::sparse_tree::{Node, SparseTree};
use pxl_spec::space::v3::{Aabb, V3};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::host::turn;
use crate::io::file::{self, Kept, Owner};
use crate::kernel::kernel;
use crate::layer::decor::build::{scatters_at, Draw};
use crate::world::access::Access;
use crate::world::change::{bump_world, wake_at, world_changes, world_edge, OBJECTS};
use crate::world::chunk::source::{chunk_sources_changed, written};
use crate::world::frame::coord::world_to_canonical;
use pxl_spec::space::zoom::{around, base_of, count, cube, from_idx, idx, index_of, nearest, of_side, units, wrap_cell, Zoom, ROUND};
use xpute_core::status::bug::OrBug;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

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
        EntryAt {
            zoom: self.zoom,
            at: wrap_cell(self.zoom, self.at),
        }
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
    u32::from(kernel().objects.tree.get(OBJECT_FINEST, wrap_cell(OBJECT_FINEST, index_of(OBJECT_FINEST, coord))).data.value)
}

/// Moves on every write.
pub fn objects_revision() -> u32 {
    world_changes().tick(OBJECTS)
}

fn apply(writes: impl IntoIterator<Item = Write>) {
    changed(kernel().objects.write(writes));
}

/// What reads a cube hears it changed.
fn changed(cubes: impl IntoIterator<Item = EntryAt>) {
    for at in cubes {
        chunk_sources_changed();
        bump_world(OBJECTS, 0);
        written(at);
    }
}

/// Whether any leaf, air included, is written in the cell `at` lies in.
pub fn holds_entry(at: &Pvf3) -> bool {
    kernel().objects.holds(CELL, index_of(CELL, at))
}

/// What stands in the cell `at` lies in, air left out.
pub fn objects_at(at: &Pvf3) -> Vec<Entry> {
    let mut out = Vec::new();
    kernel().objects.each_in(CELL, index_of(CELL, at), &mut |entry| {
        if !object_value_is_air(entry.state) {
            out.push(*entry);
        }
    });
    out
}

pub fn get_placed_object(at: EntryAt) -> Option<Entry> {
    let n = kernel().objects.tree.get(at.zoom, at.at);
    let state = u32::from(n.data.value);
    (n.zoom == at.zoom && !n.branch() && state != VOID && !object_value_is_air(state)).then_some(Entry { zoom: at.zoom, at: at.at, state })
}

struct StoreState {
    save_due_ms: Option<f64>,
    /// The saved blob and the turn it was asked on: read on a later one.
    loading: Option<(Vec<u8>, u64)>,
}

static STORE: Global<StoreState> = Global::new();

fn store_state() -> &'static mut StoreState {
    STORE.get(|| StoreState { save_due_ms: None, loading: None })
}

/// The never-written AIR entry the eraser's preview injects into the drawn
/// build where it hovers bare terrain: the cube of `size` holding `at`.
pub fn carve_entry(at: &Pvf3, size: f64) -> EntryAt {
    let zoom = of_side(size);
    EntryAt {
        zoom,
        at: wrap_cell(zoom, index_of(zoom, at)),
    }
}

// ---- Persistence ----
//
// Per tile: [tile_idx u64, x f32[], y f32[], z f32[], size f32[], value u32[]],
// positions tile-local. A placed decor's facing is derived from its position,
// so it is not stored. The layout's version is in the storage key; a blob of
// another layout is never read. Positions round-trip exactly in f32.

#[derive(Default)]
struct TileRows {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    size: Vec<f32>,
    value: Vec<u32>,
}

/// Every leaf under a node, filed by the leaf tile its corner is in.
fn collect(by_tile: &mut Vec<(IVec, TileRows)>, (zoom, at): (Zoom, [i64; 3])) {
    kernel().objects.each_in(zoom, at, &mut |entry| {
        let corner = entry.corner();
        let tile_idx = TO_XY(idx(LEAF_Z, corner.i));
        let tile_base = from_idx(LEAF_Z, tile_idx);
        let slot = match by_tile.iter().position(|(p, _)| *p == tile_idx) {
            Some(slot) => slot,
            None => {
                by_tile.push((tile_idx, TileRows::default()));
                by_tile.len() - 1
            }
        };
        let rows = &mut by_tile[slot].1;
        let local = Pvf3 {
            i: ivec::sub(corner.i, tile_base),
            f: corner.f,
        };
        rows.x.push(pvf3::to_real(pvf3::X(&local)) as f32);
        rows.y.push(pvf3::to_real(pvf3::Y(&local)) as f32);
        rows.z.push(pvf3::to_real(pvf3::Z(&local)) as f32);
        rows.size.push(entry.size() as f32);
        rows.value.push(entry.state);
    });
}

fn encode_tiles(by_tile: &[(IVec, TileRows)]) -> Result<String, MarshalError> {
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        for (tile_idx, rows) in by_tile {
            if rows.value.is_empty() {
                continue;
            }
            let value = &rows.value;
            b.branch(Some(&mut |p| {
                p.u64(Some(*tile_idx))
                    .f32_array(Some(&rows.x))
                    .f32_array(Some(&rows.y))
                    .f32_array(Some(&rows.z))
                    .f32_array(Some(&rows.size))
                    .u32_array(Some(value));
            }));
        }
    }));
    let pkt = encoder().encode(
        &tree,
        TreeEncoderOptions {
            init_cap: Some(1 << 14),
            max_cap: None,
        },
    )?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&pkt))
}

fn decode_objects(blob: &str) -> Result<(), MarshalError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(blob).map_err(|_| MarshalError::new(Errno::EINVAL))?;
    let root = TreeReader::new(&bytes)?.read_branch()?;
    let mut writes = Vec::new();
    let result = (|| {
        for k in 0..root.len {
            let tile = root.at_branch(k)?;
            let tile_base = from_idx(LEAF_Z, tile.at(0)?.get_u64()?);
            let (x, y, z) = (tile.at(1)?.get_array::<f32>()?, tile.at(2)?.get_array::<f32>()?, tile.at(3)?.get_array::<f32>()?);
            let (size, value) = (tile.at(4)?.get_array::<f32>()?, tile.at(5)?.get_array::<u32>()?);
            let base = ivec::to_v3(tile_base);
            for r in 0..value.len() {
                // Stored against its tile's corner, so an f32 resolves it.
                let corner = world_to_canonical(&[base[0] + x[r] as f64, base[1] + y[r] as f64, base[2] + z[r] as f64]);
                writes.push(write_of(&corner, size[r] as f64, value[r]));
            }
        }
        Ok(())
    })();
    apply(writes);
    result
}

pub fn load() {
    file::open_kept(Kept::Objects, Owner::Objects);
}

/// A failed open means no blob was kept: the world starts empty.
pub fn objects_opened(fd: u32, res: i32) {
    if res < 0 {
        file::close(fd);
        return;
    }
    // A blob the heap cannot hold starts the world empty, as a missing one does.
    let mut blob = Vec::new();
    if blob.try_reserve_exact(res as usize).is_err() {
        file::close(fd);
        return;
    }
    blob.resize(res as usize, 0);
    file::read(fd, &mut blob);
    file::close(fd);
    store_state().loading = Some((blob, turn()));
}

fn keep(blob: String) {
    file::write_kept(Kept::Objects, blob.into_bytes());
}

/// An unreadable blob starts the world empty, as a missing one does.
fn loaded() {
    let st = store_state();
    if !st.loading.as_ref().is_some_and(|(_, asked)| turn() > *asked) {
        return;
    }
    let (blob, _) = st.loading.take().or_bug(Errno::ENOTRECOVERABLE);
    if core::str::from_utf8(&blob).map_or(true, |raw| decode_objects(raw).is_err()) {
        // what decoded before the failure was already read
        changed(kernel().objects.clear());
    }
}

/// A save encodes the whole store, so it is put off to merge a burst of
/// writes; what is pending is flushed when the tile hides.
const SAVE_DELAY_MS: f64 = 300.0;

fn save() {
    let st = store_state();
    if st.save_due_ms.is_none() {
        st.save_due_ms = Some(now() + SAVE_DELAY_MS);
    }
}

static SAVES: Global<Notify> = Global::new();

fn saves() -> &'static mut Notify {
    SAVES.get(Notify::new)
}

pub fn flush_save_if_due() {
    loaded();
    match store_state().save_due_ms {
        Some(due) if now() >= due => saves().notify(),
        Some(due) => wake_at(due),
        None => {}
    }
}

/// Walks the store a leaf tile at a time on the turn quota. A walk the store
/// moved under is dropped and redone; the save counts as done only once
/// written, so a tile hiding mid-walk still flushes it.
pub async fn save_task() {
    loop {
        notified(saves).await;
        if store_state().save_due_ms.is_none() {
            continue;
        }
        let revision = objects_revision();
        let mut by_tile: Vec<(IVec, TileRows)> = Vec::new();
        let mut whole = true;
        for node in kernel().objects.held_at(LEAF_Z) {
            yield_if_spent(world_edge).await;
            if objects_revision() != revision {
                whole = false;
                break;
            }
            collect(&mut by_tile, node);
        }
        yield_if_spent(world_edge).await;
        if !whole || objects_revision() != revision {
            saves().notify();
            continue;
        }
        if store_state().save_due_ms.take().is_none() {
            continue;
        }
        if let Ok(blob) = encode_tiles(&by_tile) {
            keep(blob);
        }
    }
}

pub fn flush_save() {
    if store_state().save_due_ms.take().is_none() {
        return;
    }
    let mut by_tile: Vec<(IVec, TileRows)> = Vec::new();
    for node in kernel().objects.held_at(LEAF_Z) {
        collect(&mut by_tile, node);
    }
    if let Ok(blob) = encode_tiles(&by_tile) {
        keep(blob);
    }
}

// ---- Mutations ----

/// A write of VOID collapses its path back, so a redundant record never sits in a save.
fn write_object_value(coord: &Pvf3, size: f64, val: u32) {
    apply([write_of(coord, size, val)]);
    save();
}

fn write_of(coord: &Pvf3, size: f64, val: u32) -> Write {
    // Size is a power of two by construction; the nearest zoom only guards
    // against float noise.
    let zoom = of_side(size);
    Write {
        zoom,
        at: index_of(zoom, coord),
        state: val,
    }
}

/// Whether a whole-cell pick of a look only undoes a carve: the terrain must
/// show that look there, and the cell must already hold an AIR leaf (the look
/// is per column, so without this every cube stacked on matching ground was
/// dropped).
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube) -> bool {
    if size < U_PER_CELL {
        return false;
    }
    let PlaceableCube::Ground(look) = cube else { return false };
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let Some(ground) = Access::now().at(at, LEAF_Z).exact() else { return false };
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
    (size >= U_PER_CELL && object_value_is_air(read_state(at))).then(|| scatters_at(decor_num_id(decor_id), at))?
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: &Pvf3, size: f64) {
    write_object_value(at, size, cube_types::object::air as u32);
}

pub fn remove(at: EntryAt) {
    apply([Write {
        zoom: at.zoom,
        at: at.at,
        state: VOID,
    }]);
    save();
}

pub const VOID: u32 = cube_types::sentinel::void as u32;
pub const AIR: u32 = cube_types::object::air as u32;
pub const UNKNOWN: u32 = cube_types::sentinel::unknown as u32;
const _: () = assert!(VOID == 0);

/// Steps a node's box is kept in on each axis: a box is its node's, cut to
/// whole steps outward.
const BOX_STEPS: u32 = 256;

/// What a node holds as far as the tree keeps it: a leaf its value alone, a
/// branch the majority of its eight, the box everything under it fills in
/// `BOX_STEPS` of its side, and the finest zoom anything there is held at.
/// A leaf's box and finest are its own cube and zoom.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    value: u16,
    /// 0 where nothing is held.
    finest: Zoom,
    lo: [u8; 3],
    hi: [u8; 3],
}

/// Not VOID, nor the sentinel nothing writes.
fn present(value: u32) -> bool {
    value != VOID && value != UNKNOWN
}

fn leaf(value: u32) -> Held {
    Held {
        value: u16::try_from(value).or_bug(Errno::EOVERFLOW),
        ..Held::default()
    }
}

/// A node's box in its own steps and its finest; none where it holds nothing.
fn steps(n: &Node<Held>) -> Option<([u32; 3], [u32; 3], Zoom)> {
    steps_of(n.zoom, &n.data, n.branch())
}

fn steps_of(zoom: Zoom, held: &Held, branch: bool) -> Option<([u32; 3], [u32; 3], Zoom)> {
    match branch {
        true => (held.finest != 0).then(|| (held.lo.map(u32::from), held.hi.map(u32::from), held.finest)),
        false => present(u32::from(held.value)).then_some(([0; 3], [BOX_STEPS - 1; 3], zoom)),
    }
}

/// Majority over the 2×2×2 children (pxl-spec bake/fold.rs), empty counted as a value
/// and a non-voxel value as empty, so a lone cube goes at the first fold and
/// a wall keeps whole cells.
fn fold_cell(children: &[u16], zoom: Zoom) -> u16 {
    let v: [u16; 8] = core::array::from_fn(|k| if object_value_is_voxel(u32::from(children[k])) { children[k] } else { VOID as u16 });
    majority(v, zoom)
}

fn fold(kids: &[Held], branch: &[bool], zoom: Zoom) -> Held {
    let mut out = Held {
        value: fold_cell(&core::array::from_fn::<u16, 8, _>(|k| kids[k].value), zoom),
        ..Held::default()
    };
    let half = BOX_STEPS / 2;
    for k in 0..8 {
        let Some((lo, hi, finest)) = steps_of(zoom + 1, &kids[k], branch[k]) else { continue };
        let off = child_of([0; 3], k).map(|b| b as u32 * half);
        let (lo, hi) = (
            core::array::from_fn::<u8, 3, _>(|a| (off[a] + lo[a] / 2) as u8),
            core::array::from_fn::<u8, 3, _>(|a| (off[a] + hi[a] / 2) as u8),
        );
        out.lo = if out.finest == 0 { lo } else { core::array::from_fn(|a| out.lo[a].min(lo[a])) };
        out.hi = if out.finest == 0 { hi } else { core::array::from_fn(|a| out.hi[a].max(hi[a])) };
        out.finest = out.finest.max(finest);
    }
    out
}

#[derive(Clone, Copy, Debug)]
pub struct Write {
    pub zoom: Zoom,
    pub at: [i64; 3],
    pub state: u32,
}

#[derive(Default)]
pub struct Store {
    tree: SparseTree<Held>,
}

/// A node's box in the frame nearest `about`, and the rounds it was moved.
fn framed(n: &Node<Held>, about: f64) -> Option<(Aabb, [i64; 3], Zoom)> {
    let (lo, hi, finest) = steps(n)?;
    let whole = cube(n.zoom, n.at);
    let step = units(n.zoom) / f64::from(BOX_STEPS);
    let held = Aabb {
        min: core::array::from_fn(|a| whole.min[a] + f64::from(lo[a]) * step),
        max: core::array::from_fn(|a| whole.min[a] + f64::from(hi[a] + 1) * step),
    };
    let x = (whole.min[0] + whole.max[0]) / 2.0;
    let rounds = if about.is_finite() { ((nearest(x, about) - x) / ROUND).round() as i64 } else { 0 };
    let at = [n.at[0] + rounds * around(n.zoom), n.at[1], n.at[2]];
    Some((held.moved([rounds as f64 * ROUND, 0.0, 0.0]), at, finest))
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

/// A node the ray crosses: where it enters, which of the eight, its cube in
/// the ray's frame and the span it is crossed over.
type Met = (f64, usize, [i64; 3], (f64, f64));

impl Store {
    /// Every node whose box meets `query`, in `query`'s frame, a branch
    /// entered where `enter` says so.
    fn descend(&self, query: &Aabb, enter: &mut impl FnMut(&Node<Held>, [i64; 3], &Aabb, Zoom) -> bool) {
        let about = about(query);
        self.tree.visit(&self.tree.roots(), &mut |n| {
            let Some((held, at, finest)) = framed(n, about) else { return false };
            held.meet(*query).is_some() && enter(n, at, &held, finest)
        });
    }

    /// A box holding every entry inside `query`, never none where there is
    /// some; a leaf's is cut to `query` and may include air.
    pub fn bound_in(&self, query: &Aabb) -> Option<Aabb> {
        let mut out: Option<Aabb> = None;
        self.descend(query, &mut |n, at, held, _| {
            let whole = !n.branch() || query.contains(cube(n.zoom, at));
            if let Some(b) = held.meet(*query).filter(|_| whole) {
                out = Some(out.map_or(b, |o| o.join(b)));
            }
            !whole
        });
        out
    }

    /// The finest zoom any entry inside `query` is held at: a node wholly
    /// inside answers for all under it, never less than what is there.
    pub fn finest_in(&self, query: &Aabb) -> Option<Zoom> {
        let mut out: Option<Zoom> = None;
        self.descend(query, &mut |n, at, _, finest| {
            let whole = !n.branch() || query.contains(cube(n.zoom, at));
            if whole {
                out = Some(out.map_or(finest, |f| f.max(finest)));
            }
            !whole
        });
        out
    }

    /// Every leaf meeting the cube at `zoom`, X in the cube's frame.
    pub fn each_in(&self, zoom: Zoom, at: [i64; 3], visit: &mut dyn FnMut(&Entry)) {
        let about = about(&cube(zoom, at));
        let from = self.tree.get(zoom, wrap_cell(zoom, at));
        self.tree.visit(&[from], &mut |n| {
            let Some((_, at, _)) = framed(n, about) else { return false };
            if !n.branch() {
                visit(&Entry {
                    zoom: n.zoom,
                    at,
                    state: u32::from(n.data.value),
                });
            }
            n.branch()
        });
    }

    /// Every entry whose cube meets `query`, X in `query`'s frame.
    pub fn entries_in(&self, query: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.descend(query, &mut |n, at, _, _| {
            if !n.branch() {
                visit(&Entry {
                    zoom: n.zoom,
                    at,
                    state: u32::from(n.data.value),
                });
            }
            n.branch()
        });
    }

    /// The nodes at `zoom` holding anything, or the leaves above them, as the tree names them.
    pub fn held_at(&self, zoom: Zoom) -> Vec<(Zoom, [i64; 3])> {
        let mut out = Vec::new();
        self.tree.visit(&self.tree.roots(), &mut |n| {
            if steps(n).is_none() {
                return false;
            }
            if !n.branch() || n.zoom == zoom {
                out.push((n.zoom, n.at));
                return false;
            }
            true
        });
        out
    }

    /// Whether anything, air included, is held in the cube at `zoom`.
    pub fn holds(&self, zoom: Zoom, at: [i64; 3]) -> bool {
        steps(&self.tree.get(zoom, wrap_cell(zoom, at))).is_some()
    }

    /// The cell over the highest entry in the column of cell `[x, y]`.
    pub fn ceiling(&self, [x, y]: [i64; 2]) -> Option<i64> {
        let side = units(CELL);
        let column = Aabb {
            min: [x as f64 * side, y as f64 * side, REACH.min[2]],
            max: [(x + 1) as f64 * side, (y + 1) as f64 * side, REACH.max[2]],
        };
        self.bound_in(&column).map(|b| (b.max[2] / side).ceil() as i64)
    }

    /// Every cell of `cell_zoom` that may hold objects along the ray, in ray
    /// order, until `each` stops it. A node is entered only where the ray
    /// crosses its box grown to whole cells of `cell_zoom`.
    pub fn along_ray(&self, origin: V3, direction: V3, span: (f64, f64), cell_zoom: Zoom, each: &mut dyn FnMut([i64; 3]) -> bool) {
        let ray = (origin, direction);
        let roots = self.tree.roots();
        self.along(ray, span, cell_zoom, &roots, each);
    }

    fn along(&self, ray: (V3, V3), span: (f64, f64), cell_zoom: Zoom, nodes: &[Node<Held>], each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
        let side = units(cell_zoom);
        let mut met: [Met; 8] = [(0.0, 0, [0; 3], (0.0, 0.0)); 8];
        let mut n = 0;
        for (k, node) in nodes.iter().enumerate() {
            let Some((held, at, _)) = framed(node, ray.0[0]) else { continue };
            if let Some(s) = slab(ray, &held.snapped_out(side), span) {
                met[n] = (s.0, k, at, s);
                n += 1;
            }
        }
        met[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        met[..n].iter().any(|&(_, k, at, s)| {
            let node = &nodes[k];
            if node.zoom == cell_zoom {
                return each(at);
            }
            match self.tree.children(node) {
                Some(kids) => self.along(ray, s, cell_zoom, &kids, each),
                // A leaf coarser than the cell is every cell of it.
                None => cells_along(ray, s, side, each),
            }
        })
    }

    /// What the objects fold to in the cells `from..to` of `target`, at every
    /// height, with `hide` left out.
    pub fn fold_at(&self, target: Zoom, from: [i64; 2], to: [i64; 2], hide: Option<EntryAt>, out: &mut HashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        out.clear();
        let mut refused = Ok(());
        let inside = |x: i64, y: i64| (from[0]..to[0]).contains(&x) && (from[1]..to[1]).contains(&y);
        let side = units(target);
        let query = Aabb {
            min: [from[0] as f64 * side, from[1] as f64 * side, REACH.min[2]],
            max: [to[0] as f64 * side, to[1] as f64 * side, REACH.max[2]],
        };
        self.descend(&query, &mut |n, at, _, _| {
            if n.branch() && n.zoom < target {
                return true;
            }
            let v = u32::from(n.data.value);
            if !object_value_is_voxel(v) || refused.is_err() {
                return false;
            }
            // A leaf coarser than the cell is each of its cells.
            let cells = i64::from(count(target, n.zoom));
            let [x0, y0, z0] = first_descendant_of(at, u32::from(target - n.zoom));
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
        refused?;
        if let Some((cell, v)) = hide.and_then(|h| self.hidden(h, target)) {
            let x = from[0] + (cell[0] - from[0]).rem_euclid(around(target));
            if inside(x, cell[1]) {
                let cell = [x, cell[1], cell[2]];
                match v {
                    0 => {
                        out.remove(&cell);
                    }
                    v => try_insert(out, cell, u32::from(v))?,
                }
            }
        }
        Ok(())
    }

    /// The leaf holding the cube at `zoom`, X in the asker's frame, or the
    /// cube itself as its eight fold to, and whether it is that fold.
    pub fn node(&self, zoom: Zoom, at: [i64; 3]) -> (Entry, bool) {
        let n = self.tree.get(zoom, wrap_cell(zoom, at));
        let entry = Entry {
            zoom: n.zoom,
            at: at.map(|c| c >> (zoom - n.zoom)),
            state: u32::from(n.data.value),
        };
        (entry, n.branch())
    }

    /// VOID where nothing; a non-voxel value is returned as is.
    pub fn folded_value(&self, zoom: Zoom, at: [i64; 3]) -> u16 {
        self.tree.get(zoom, wrap_cell(zoom, at)).data.value
    }

    /// What the cell of `target` over `hide` folds to without it.
    fn hidden(&self, hide: EntryAt, target: Zoom) -> Option<([i64; 3], u16)> {
        let n = self.tree.get(hide.zoom, hide.at);
        (n.zoom == hide.zoom && !n.branch()).then_some(())?;
        let (mut at, mut zoom, mut v) = (hide.at, hide.zoom, VOID as u16);
        while zoom > target {
            let parent = parent_of(at);
            let children: [u16; 8] = core::array::from_fn(|k| {
                let c = child_of(parent, k);
                if c == at {
                    v
                } else {
                    self.folded_value(zoom, c)
                }
            });
            (at, zoom) = (parent, zoom - 1);
            v = fold_cell(&children, zoom);
        }
        Some((at, v))
    }

    /// Applies every write; returns the cubes that changed.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> Vec<EntryAt> {
        let mut changed = Vec::new();
        for Write { zoom, at, state } in writes {
            xpute_core::ensure!((1..=OBJECT_FINEST).contains(&zoom), ENOTRECOVERABLE, zoom);
            let at = wrap_cell(zoom, at);
            if self.tree.set(zoom, at, leaf(state), &fold) {
                changed.push(EntryAt { zoom, at });
            }
        }
        changed
    }

    /// Returns the cubes held.
    pub fn clear(&mut self) -> Vec<EntryAt> {
        let held = self.held_at(OBJECT_FINEST).into_iter().map(|(zoom, at)| EntryAt { zoom, at }).collect();
        self.tree = SparseTree::default();
        held
    }
}

fn try_insert(out: &mut HashMap<[i64; 3], u32>, k: [i64; 3], v: u32) -> Result<(), TryReserveError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))?;
    }
    out.insert(k, v);
    Ok(())
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
