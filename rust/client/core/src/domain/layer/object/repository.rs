// pxl-core/domain/layer/object/repository.rs

//! The object layer's repository: where placed objects are kept and where
//! they come from. It is the one way the layer's records are read and
//! written. What the view asks for arrives here, and each edit leaves from
//! here, every write as `<key, asset_id, ext>`. The objects are kept by their
//! source, a mock or a server (ts/client/src/service/sys/fs/object.ts), after OBJECT
//! (file.json5 `path`); this file reads what the source serves and sends it
//! the edits.
//!
//! The source keeps the objects one shard at a time (pxl-spec
//! `SHARD_ZOOM`), in one file for each shard named by the shard's quadkey,
//! as a baked tile is named by its own. It also keeps the top of their tree
//! (file.json5 `path`, (OBJECT, TOP)): the fold of each node at `SHARD_ZOOM` that holds
//! objects. The top is read first, and its folds stand in the store for the
//! shards not read yet (store.rs `Store::set_fold`), so a view from afar
//! draws from them.
//!
//! A shard is read as a baked tile is, through the request scheduler
//! (sys/io/request.rs) and the arrival of fetches (sys/io/fetch.rs). It is
//! wanted where the view asks for it (`want`) and the top lists it, and its
//! leaves go into the layer's one store (store.rs) in place of its fold. A
//! shard the top does not list is taken as empty without asking. Once read,
//! a shard stays read.
//!
//! The writes the store takes go up as edits in batches (pxl:core object
//! `apply`), and the source applies them to its shards and its top; a batch
//! the source fails to apply stays (`save`). A file that does not decode is
//! refused: the core sets it aside (sys/io/file.rs `set_aside`), and reads it
//! again once it is (`on_set_aside`).

use crate::spec::step::InvalidationReason;
use fnv::FnvHashSet;
use pxl_engine::object::high;
use pxl_global::Global;
use pxl_spec::asset::marker::{MARKER_TEXT_CHARS, text_fits};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{CUBE_ZOOM, SHARD_ZOOM, units};
use pxl_spec::space::hierarchy::morton::{key_of_name, name, node_of};
use pxl_spec::space::layer::area::shard_over;
use pxl_spec::space::layer::demand::GROUND_DEMAND_TILE_CAP;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_guest::abi::fs::types::ErrorCode;
use xpute_guest::clock::now;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;
use xpute_kit::wire::xtp::{NodeView, TreeEncoderConfig, TreeReader, TreeView, encoder};

use super::store::{object, object_mut};
use crate::abi::object;
use crate::domain::layer::object::ext::Ext;
use crate::domain::layer::object::store::{Write, apply, note_damage};
use crate::spec;
use crate::spec::file::{format, top, write as column};
use crate::sys::io::body::Body;
use crate::sys::io::demand::{Order, PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::{Exit, Failure};
use crate::sys::io::file;
use crate::sys::io::request::declare;
use crate::sys::sched::{executor, invalidate};
use pxl_spec::object::{Class, class_of};

/// The most files wanted at once. Before the top is read, only the top is
/// wanted. After that, the wanted files are the shards within the ground's
/// demand at the shard zoom around the eye (domain/view/demand.rs
/// `object_step`), which are no more than one zoom's demand of ground tiles
/// (pxl-spec layer/demand.rs `demand_tiles`).
pub(crate) const WANT_CAP: u32 = GROUND_DEMAND_TILE_CAP;

/// The id that names the top among the wanted files; every other wanted id
/// is a shard's.
const TOP: u64 = file::TOP_ID;

// The source and the core must agree on the shard zoom (file.json5
// `SHARD_ZOOM`).
const _: () = assert!(spec::file::SHARD_ZOOM == SHARD_ZOOM);

#[derive(Default)]
struct State {
    /// The shards the top lists, by id; none until the top is read.
    top_shards: Option<FnvHashSet<u64>>,
    /// The shards read, by id. Their leaves are in the store.
    read_shards: FnvHashSet<u64>,
    /// The refused files after OBJECT, by id. Each stays refused until it is
    /// set aside.
    refused_files: Vec<u64>,
    /// The writes the store took that the source has not applied, in order.
    unsent: Vec<Write>,
    /// Whether a batch of `unsent` is on its way to the source.
    applying: bool,
    /// How many of `unsent` the last batch the source failed to apply held;
    /// they go again with the next write.
    failed_cnt: usize,
    /// The files `want` asks for, by id; the buffer is reused across frames.
    wants: Vec<(u64, Order)>,
    /// A counter that advances whenever something `want` reads changes: the
    /// top or a shard is read, or a file is refused or set aside.
    revision: u32,
    /// The changes waiting for the store, in order, held while the host
    /// holds the store's columns (`change`).
    changes: Vec<Change>,
}

/// A change to the store: a shard's fold to hold, a shard's leaves to put in
/// place of its fold, or an edit made on this device, which is also sent to
/// the source when it changed what is placed.
enum Change {
    Fold([i32; 2], u32),
    Shard([i32; 2], Vec<Write>),
    Edit(Write),
}

/// Makes `c` at once when the host holds none of the store's columns
/// (store.rs `Store::is_in_flight`) and no other change waits. Otherwise it
/// holds `c` for `step`, in order. Only writers of the store wait.
fn change(c: Change) {
    let st = state();
    if st.changes.is_empty() && !object().is_in_flight() {
        make(c);
    } else {
        st.changes.push(c);
        invalidate(InvalidationReason::Pending);
    }
}

fn make(c: Change) {
    match c {
        Change::Fold(at, word) => note_damage(&[object_mut().set_fold(at, word)]),
        Change::Shard(at, writes) => {
            if let Some(changed) = object_mut().clear_fold(at) {
                note_damage(&[changed]);
            }
            apply(writes);
        }
        Change::Edit(write) => {
            if apply([write.clone()]) {
                send(write);
            }
        }
    }
}

/// Applies an edit made on this device to the store (`change`), and sends it
/// to the source when it changed what is placed.
pub(crate) fn edit(write: Write) {
    change(Change::Edit(write));
}

impl State {
    /// Whether all of shard `id`'s leaves are in the store: the shard was
    /// read, or the top was read and does not list it.
    fn is_read(&self, id: u64) -> bool {
        self.top_shards
            .as_ref()
            .is_some_and(|l| !l.contains(&id) || self.read_shards.contains(&id))
    }
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(State::default)
}

/// The shard that leaf `key` lies in. Every leaf is at the shard zoom or
/// finer (store.rs `Store::write`).
fn shard_of(key: u128) -> Quadrant {
    let (zoom, at) = node_of::<3>(key);
    let (zoom, at) = shard_over(zoom, [at[0], at[1]]);
    Quadrant::new(zoom, at)
}

/// Whether every shard under `space` was read. Only then is a write there
/// made over what the shard's file holds (domain/query/limit.rs
/// `editable`).
pub(crate) fn holds(space: &Aabb) -> bool {
    let side = units(SHARD_ZOOM);
    let span = |k: usize| {
        let first = (space.min[k] / side).floor() as i32;
        first..=((space.max[k] / side).ceil() as i32 - 1).max(first)
    };
    let st = state();
    span(1).all(|y| span(0).all(|x| st.is_read(Quadrant::new(SHARD_ZOOM, [x, y]).id())))
}

/// Declares the files wanted for `asked`, the shards the view asks for
/// (domain/view/demand.rs `object_step`), at most `WANT_CAP` of them and each in
/// its `order`. Before the top is read, only the top is wanted, first. After
/// that, the wanted files are the asked shards that the top lists and that
/// are not read yet, and they replace what was wanted before. A refused file
/// is not wanted until it is set aside.
pub(crate) fn want(asked: impl Iterator<Item = Quadrant>, order: impl Fn(Quadrant) -> Order) {
    let st = state();
    st.wants.clear();
    if st.top_shards.is_none() {
        if !st.refused_files.contains(&TOP) {
            st.wants.push((TOP, Order::FIRST));
        }
    } else {
        for shard in asked {
            let id = shard.id();
            if !st.is_read(id) && !st.refused_files.contains(&id) {
                st.wants.push((id, order(shard)));
            }
        }
        st.wants.sort_unstable_by_key(|&(id, _)| id);
    }
    declare(Stream::Object, &st.wants);
}

pub(crate) fn want_again() {
    declare(Stream::Object, &state().wants);
}

/// The repository's revision for `want`. Two readings are equal only if no
/// top or shard was read and no file was refused or set aside between them.
pub(crate) fn revision() -> u32 {
    state().revision
}

fn changed() {
    let st = state();
    st.revision = st.revision.wrapping_add(1);
}

/// Takes in a file after OBJECT that arrived, the top or a shard, or the
/// news that it is absent (`admit`).
pub(crate) fn on_arrived(id: u64, pkt: Option<Body>, phases: &mut PhaseList) -> Result<(), Exit> {
    let at = now();
    let result = admit(id, pkt.as_deref());
    phases.push(TaskPhaseName::DecodeMs, now() - at);
    result.map_err(|e| Exit::Failed(Failure::from(e)))
}

/// Takes in file `id` after OBJECT with its `bytes`, or none where it is
/// absent. The top's folds are held for the shards not read yet, and a
/// shard's leaves go into the store in place of its fold. A file that does
/// not decode leaves the store as it was and is refused.
fn admit(id: u64, bytes: Option<&[u8]>) -> Result<(), XputeError> {
    let result = match bytes {
        // An absent top means that no shard holds any object.
        None if id == TOP => {
            on_top(Vec::new());
            Ok(())
        }
        None => {
            on_shard(id, Vec::new());
            Ok(())
        }
        Some(b) if id == TOP => decode_top(b).map(on_top),
        Some(b) => decode_shard(Quadrant::of_id(id), b).map(|w| on_shard(id, w)),
    };
    if result.is_err() && !state().refused_files.contains(&id) {
        state().refused_files.push(id);
        file::set_aside(file::Refused::Object(id));
        changed();
        invalidate(InvalidationReason::Pending);
    }
    result
}

fn on_top(rows: Vec<(u64, u32)>) {
    let mut listed = FnvHashSet::default();
    for (shard, word) in rows {
        listed.insert(shard);
        if !state().read_shards.contains(&shard) {
            change(Change::Fold(Quadrant::of_id(shard).at, word));
        }
    }
    state().top_shards = Some(listed);
    changed();
}

fn on_shard(id: u64, writes: Vec<Write>) {
    change(Change::Shard(Quadrant::of_id(id).at, writes));
    state().read_shards.insert(id);
    changed();
}

/// Refused file `id` was set aside, so it is wanted again as a new file.
/// Returns false where the file was not refused.
pub(crate) fn on_set_aside(id: u64) -> bool {
    let st = state();
    let Some(i) = st.refused_files.iter().position(|&n| n == id) else {
        return false;
    };
    st.refused_files.remove(i);
    st.revision = st.revision.wrapping_add(1);
    // Drop the failed want, so that the next declaration starts a new one.
    st.wants.retain(|&(n, _)| n != id);
    declare(Stream::Object, &st.wants);
    invalidate(InvalidationReason::Pending);
    true
}

// A shard's file and an edit batch share one layout: the format byte
// (file.json5 `format`), then rows of writes `<key, asset_id, ext>` as file.json5
// `write` lays them out. A shard's file holds each of its leaves as the
// write that made it; a batch holds its writes in the order they were made.
// A file of any other format is never read.

fn encode_writes(format: u8, writes: &[Write]) -> Result<Vec<u8>, XputeError> {
    let (mut keys, mut asset_ids) = (Vec::new(), Vec::new());
    let (mut text_rows, mut texts) = (Vec::new(), Vec::new());
    for w in writes {
        match &w.ext {
            Some(Ext::MarkerText(t)) => {
                text_rows.push(asset_ids.len() as u32);
                texts.push(t.as_str());
            }
            None => {}
        }
        keys.push(name::<3>(w.at));
        asset_ids.push(w.asset_id);
    }
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        const _: () = assert!(
            column::KEY == 0 && column::ASSET_ID == 1 && column::TEXT_ROW == 2 && column::TEXT == 3
        );
        b.strs(Some(&keys))
            .u32_array(Some(&asset_ids))
            .u32_array(Some(&text_rows))
            .strs(Some(&texts));
    }));
    let pkt = encoder().encode(
        &tree,
        TreeEncoderConfig {
            init_cap: 1 << 14,
            max_cap: None,
        },
    )?;
    let mut file = Vec::with_capacity(1 + pkt.len());
    file.push(format);
    file.extend_from_slice(&pkt);
    Ok(file)
}

/// The tree packet inside file `bytes` of `format`. A file of another format
/// is an error.
fn pkt(format: u8, bytes: &[u8]) -> Result<Vec<u8>, XputeError> {
    match bytes.split_first() {
        // The packet is copied because `TreeReader` needs it to start on a
        // word, and an allocation of its own does.
        Some((&f, pkt)) if f == format => Ok(pkt.to_vec()),
        _ => Err(XputeError::new(Errno::EBADMSG)),
    }
}

/// The writes that file `bytes` of `format` holds. Each record must be of
/// its leaf's class.
fn decode_writes(format: u8, bytes: &[u8]) -> Result<Vec<Write>, XputeError> {
    let pkt = pkt(format, bytes)?;
    let root = TreeReader::new(&pkt)?.read_branch()?;
    let (keys, asset_ids) = (
        root.at(column::KEY)?.get_strs()?,
        root.at(column::ASSET_ID)?.get_array::<u32>()?,
    );
    let (text_rows, texts) = (
        root.at(column::TEXT_ROW)?.get_array::<u32>()?,
        root.at(column::TEXT)?.get_strs()?,
    );
    if keys.len() as usize != asset_ids.len() || text_rows.len() != texts.len() as usize {
        return Err(XputeError::new(Errno::EINVAL));
    }
    let mut ext: Vec<Option<Ext>> = vec![None; asset_ids.len()];
    for (r, t) in text_rows.iter().zip(texts.iter()) {
        let r = *r as usize;
        // A record of a class other than its leaf's means the file is not in
        // this layout.
        if asset_ids.get(r).and_then(|&v| class_of(v)) != Some(Class::MarkerText) {
            return Err(XputeError::new(Errno::EINVAL));
        }
        // A marker's text past its cap means the same.
        if !text_fits(t, MARKER_TEXT_CHARS) {
            return Err(XputeError::new(Errno::EINVAL));
        }
        ext[r] = Some(Ext::MarkerText(t.to_string()));
    }
    keys.iter()
        .enumerate()
        .map(|(r, key)| {
            Ok(Write {
                at: key_of_name::<3>(key).ok_or(XputeError::new(Errno::EINVAL))?,
                asset_id: asset_ids[r],
                ext: ext[r].take(),
            })
        })
        .collect()
}

/// The leaves in `bytes`, the file of `shard`, all or none. Each leaf must
/// lie in `shard`, at a zoom the store holds, with an id that fits a leaf.
fn decode_shard(shard: Quadrant, bytes: &[u8]) -> Result<Vec<Write>, XputeError> {
    let writes = decode_writes(format::OBJECT, bytes)?;
    let fits = |w: &Write| {
        (CUBE_ZOOM..=OBJECT_FINEST).contains(&node_of::<3>(w.at).0)
            && shard_of(w.at) == shard
            && u16::try_from(w.asset_id).is_ok()
    };
    if !writes.iter().all(fits) {
        return Err(XputeError::new(Errno::EINVAL));
    }
    Ok(writes)
}

/// The top's rows `<shard, fold>`, as file.json5 `top` lays them out, all or
/// none. Each shard must be a tile at `SHARD_ZOOM`, and the filled share of
/// its cube that its fold gives must be at most the whole.
fn decode_top(bytes: &[u8]) -> Result<Vec<(u64, u32)>, XputeError> {
    let pkt = pkt(format::OBJECT_TOP, bytes)?;
    let root = TreeReader::new(&pkt)?.read_branch()?;
    let (shard, fold) = (
        root.at(top::SHARD)?.get_strs()?,
        root.at(top::FOLD)?.get_array::<u32>()?,
    );
    if shard.len() as usize != fold.len() {
        return Err(XputeError::new(Errno::EINVAL));
    }
    shard
        .iter()
        .enumerate()
        .map(|(r, quadkey)| {
            Quadrant::of_key(quadkey)
                .filter(|t| t.zoom == SHARD_ZOOM && high(fold[r]) <= FULL)
                .map(|t| (t.id(), fold[r]))
                .ok_or(XputeError::new(Errno::EINVAL))
        })
        .collect()
}

/// Queues a write the store took, to go up as an edit with the frame's other
/// writes (`step`).
pub(crate) fn send(write: Write) {
    state().unsent.push(write);
}

pub(crate) fn step() {
    // Make every change held while the host held the store's columns, now
    // that it holds none. The host cannot take them back during the loop.
    if !state().changes.is_empty() && !object().is_in_flight() {
        let mut changes = core::mem::take(&mut state().changes);
        for c in changes.drain(..) {
            make(c);
        }
        state().changes = changes;
    }
    save();
}

/// Sends the writes the source has not applied as one edit batch (pxl:core
/// object `apply`), one batch at a time. A batch the source fails to apply
/// stays, and goes again with the next write; one the source keeps nothing
/// of (`read-only`) or cannot read (`bad-message`), or that does not encode,
/// is dropped.
pub(crate) fn save() {
    let st = state();
    if st.applying || st.unsent.len() <= st.failed_cnt {
        return;
    }
    let Ok(batch) = encode_writes(format::OBJECT_EDIT, &st.unsent) else {
        st.unsent.clear();
        st.failed_cnt = 0;
        return;
    };
    st.applying = true;
    executor::spawner().spawn(apply_batch(batch, st.unsent.len()).or_bug(Errno::ENOSPC));
}

/// Applies `batch`, the first `n` writes of `unsent`, and drops them where
/// the source applied them, keeps nothing or cannot read them.
#[embassy_executor::task]
async fn apply_batch(batch: Vec<u8>, n: usize) {
    let (_, applied) = object::apply(batch).await;
    let st = state();
    st.applying = false;
    match applied {
        Ok(Ok(()) | Err(ErrorCode::ReadOnly | ErrorCode::BadMessage)) => {
            st.unsent.drain(..n);
            st.failed_cnt = 0;
        }
        _ => st.failed_cnt = n,
    }
    invalidate(InvalidationReason::Completion);
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod test;
