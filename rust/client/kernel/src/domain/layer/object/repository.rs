// pxl-kernel/domain/layer/object/repository.rs

//! Where placed objects are kept and come from, the one way the object
//! layer's records are read and written: what is asked for arrives and an
//! edit is sent, each write `<key, id, ext>`. Who keeps them is their
//! source's, a host's or a server's (ts/client/src/driver/namespace/object.ts),
//! below OBJECT (host.json `root`); this file reads what it serves and sends
//! it edits.
//!
//! The source keeps them a shard at a time (pxl-engine `SHARD_ZOOM`), a
//! file a shard named by the shard's subject, as a bake's tile is named by
//! its own, and the top of their tree (host.json `object.TOP`): what each node
//! at `SHARD_ZOOM` holding objects folds to. The top is read first, and its
//! folds are held in the store where their shards are not read
//! (store.rs `Store::set_fold`), so a view from afar draws from it. A shard
//! is read as a bake's tile is, through the request scheduler
//! (sys/io/request.rs) and the fetches' arrival (sys/io/fetch.rs), wanted
//! where the view asks for it (`want`) and the top lists it, and its leaves
//! go into the layer's one store (store.rs) in place of its folds; a shard
//! the top does not list is read as empty without asking. A shard read
//! stays read.
//!
//! The writes the store takes go up as edits, batched (host.json
//! `object.EDITS`), which the source applies to its shards and its top. A
//! file that does not decode is refused, named to the host to set aside
//! (`refused_files`), and read again once it says so (`on_emptied`).

use fnv::FnvHashSet;
use pxl_engine::answer::FULL;
use pxl_engine::area::shard_over;
use pxl_engine::overwrite;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{units, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{is_key, node_of};
use pxl_spec::space::layer::demand::GROUND_DEMAND_TILES;
use pxl_spec::space::layer::tile::{tile_at, Tile};
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::heap::Block;
use xpute_guest::sched::edge::{notified, Notify};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use super::store::object;
use crate::domain::layer::object::ext::Ext;
use crate::domain::layer::object::store::{apply, note_written, Write};
use crate::sys::io::demand::{Ask, Order, Phases, Priority, Stream, TaskPhaseName};
use crate::sys::io::failure::Failure;
use crate::sys::io::fetch::{body, release};
use crate::sys::io::file;
use crate::sys::io::host_spec::{self, format, object as object_file, root, top, write as column};
use crate::sys::io::request::declare;
use crate::sys::sched::edge::{request_world_tick, wake_at};
use pxl_spec::object::ext::{class_of, Class};

/// The most files wanted at once: the shards within the ground's demand at
/// the shard's zoom about the eye (domain/view/demand.rs `object_step`), as
/// many as a zoom's demand names of the ground's tiles (pxl-spec
/// layer/demand.rs `demand_tiles`), or the top alone, before any shard.
pub const WANTS: u32 = GROUND_DEMAND_TILES;

/// The top's number below OBJECT, and the edits'.
const TOP: u64 = object_file::TOP as u64;
const EDITS: u64 = object_file::EDITS as u64;

// What the source reads a shard's zoom as is this one's (host.json `SHARD_ZOOM`).
const _: () = assert!(host_spec::SHARD_ZOOM == SHARD_ZOOM);

/// A shard's nodes at `SHARD_ZOOM` on Z, below sea level and above: the
/// world's height spans the two (pxl-spec space/coord/world.rs `EXTENT`).
const SHARD_Z: [i32; 2] = [-1, 0];

#[derive(Default)]
struct State {
    /// The shards the top lists, by subject; none until it is read.
    top_shards: Option<FnvHashSet<u64>>,
    /// The shards read, by subject: their leaves are in the store.
    read: FnvHashSet<u64>,
    /// The files below OBJECT refused, by number, until the host says each
    /// is empty or its source's own again.
    refused_files: Vec<u64>,
    /// The writes the store took since the last batch went up, in order.
    unsent: Vec<Write>,
    save_due_ms: Option<f64>,
    /// The files `want` asks for, by number; reused across turns.
    wants: Vec<(u64, Ask)>,
}

impl State {
    /// Whether the shard `subject`'s leaves are all in the store: read, or
    /// listed by no top read.
    fn is_read(&self, subject: u64) -> bool {
        self.top_shards
            .as_ref()
            .is_some_and(|l| !l.contains(&subject) || self.read.contains(&subject))
    }
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(State::default)
}

/// The shard the leaf `key` names lies in. Every leaf is a shard or finer
/// (store.rs `Store::write`).
fn shard_of(key: u128) -> Tile {
    let (zoom, at) = node_of::<3>(key);
    let (zoom, at) = shard_over(zoom, [at[0], at[1]]);
    tile_at(zoom, at)
}

/// Whether every shard under `space` was read, so a write there stands
/// over what its file held (domain/query/limit.rs `editable`).
pub fn holds(space: &Aabb) -> bool {
    let side = units(SHARD_ZOOM);
    let span = |k: usize| {
        let first = (space.min[k] / side).floor() as i32;
        first..=((space.max[k] / side).ceil() as i32 - 1).max(first)
    };
    let st = state();
    span(1).all(|y| span(0).all(|x| st.is_read(tile_at(SHARD_ZOOM, [x, y]).subject())))
}

/// The shards the view asks for (domain/view/demand.rs `object_step`), each
/// with its ask, no more than `WANTS`: before the top is read, the top
/// alone; then those it lists not yet read, in place of what was wanted
/// before. A file refused is not wanted until the host empties it.
pub fn want(asked: impl Iterator<Item = (Tile, Ask)>) {
    let st = state();
    st.wants.clear();
    if st.top_shards.is_none() {
        if !st.refused_files.contains(&TOP) {
            let ask = Ask {
                priority: Priority::Demand,
                order: Order::FIRST,
            };
            st.wants.push((TOP, ask));
        }
    } else {
        for (shard, ask) in asked {
            let subject = shard.subject();
            if !st.is_read(subject) && !st.refused_files.contains(&subject) {
                st.wants.push((subject, ask));
            }
        }
        st.wants.sort_unstable_by_key(|&(subject, _)| subject);
    }
    declare(Stream::object, &st.wants);
}

/// A file below OBJECT arrived, the top's or a shard's, or its absence
/// (`admit`).
pub fn on_arrived(number: u64, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    let at = now();
    let result = admit(number, block.map(body));
    release(block);
    phases.push(TaskPhaseName::decode_ms, now() - at);
    result.map_err(Failure::from)
}

/// The file `number` below OBJECT, `bytes`, or none: the top's folds held
/// where their shards are not read, a shard's leaves into the store in
/// place of its folds. One that does not decode leaves the store as it was
/// and is refused.
fn admit(number: u64, bytes: Option<&[u8]>) -> Result<(), MarshalError> {
    let result = match bytes {
        // No top: no shard holds any.
        None if number == TOP => {
            on_top(Vec::new());
            Ok(())
        }
        None => {
            on_shard(number, Vec::new());
            Ok(())
        }
        Some(b) if number == TOP => decode_top(b).map(on_top),
        Some(b) => decode_shard(Tile::of_subject(number), b).map(|w| on_shard(number, w)),
    };
    if result.is_err() && !state().refused_files.contains(&number) {
        state().refused_files.push(number);
        request_world_tick();
    }
    result
}

/// The top read: the shards it lists, and each fold of a shard not read
/// held in the store.
fn on_top(rows: Vec<(u64, i32, u32)>) {
    let st = state();
    let mut listed = FnvHashSet::default();
    let mut changed = Vec::new();
    for (shard, z, word) in rows {
        listed.insert(shard);
        if !st.read.contains(&shard) {
            let at = Tile::of_subject(shard).at;
            changed.push(object().set_fold([at[0], at[1], z], word));
        }
    }
    st.top_shards = Some(listed);
    if !changed.is_empty() {
        note_written(&changed);
    }
}

/// The shard `subject` read: its folds let go and its leaves written.
fn on_shard(subject: u64, writes: Vec<Write>) {
    let at = Tile::of_subject(subject).at;
    let changed: Vec<Aabb> = SHARD_Z
        .iter()
        .filter_map(|&z| object().clear_fold([at[0], at[1], z]))
        .collect();
    if !changed.is_empty() {
        note_written(&changed);
    }
    apply(writes);
    state().read.insert(subject);
}

/// The files refused and not yet emptied, by number below OBJECT.
pub fn refused_files() -> &'static [u64] {
    &state().refused_files
}

/// The host set the refused file `number` aside: it is wanted again, as
/// new. False where it was not refused.
pub fn on_emptied(number: u64) -> bool {
    let st = state();
    let Some(i) = st.refused_files.iter().position(|&n| n == number) else {
        return false;
    };
    st.refused_files.remove(i);
    // Its failed want goes, so the next is a new one.
    st.wants.retain(|&(n, _)| n != number);
    declare(Stream::object, &st.wants);
    request_world_tick();
    true
}

// A shard's file and an edit batch: the format byte (host.json `format`),
// then the rows of writes `<key, id, ext>` as host.json `write` lays them
// out: a shard's every leaf as the write that made it, a batch's writes in
// the order made. A file of another format is never read.

/// `writes` as a file of `format`.
fn encode_writes(format: u8, writes: &[Write]) -> Result<Vec<u8>, MarshalError> {
    let (mut hi, mut lo, mut id) = (Vec::new(), Vec::new(), Vec::new());
    let (mut text_rows, mut texts) = (Vec::new(), Vec::new());
    for w in writes {
        match &w.ext {
            Some(Ext::MarkerText(t)) => {
                text_rows.push(id.len() as u32);
                texts.push(t.as_str());
            }
            None => {}
        }
        hi.push((w.key >> 64) as u64);
        lo.push(w.key as u64);
        id.push(w.id);
    }
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        const _: () = assert!(
            column::KEY_HI == 0
                && column::KEY_LO == 1
                && column::ID == 2
                && column::TEXT_ROW == 3
                && column::TEXT == 4
        );
        b.u64_array(Some(&hi))
            .u64_array(Some(&lo))
            .u32_array(Some(&id))
            .u32_array(Some(&text_rows))
            .strs(Some(&texts));
    }));
    let pkt = encoder().encode(
        &tree,
        TreeEncoderOptions {
            init_cap: 1 << 14,
            max_cap: None,
        },
    )?;
    let mut file = Vec::with_capacity(1 + pkt.len());
    file.push(format);
    file.extend_from_slice(&pkt);
    Ok(file)
}

/// The tree packet of a file of `format`, `bytes`; none of another format.
fn packet(format: u8, bytes: &[u8]) -> Result<Vec<u8>, MarshalError> {
    match bytes.split_first() {
        // A packet starts on a word (`TreeReader`), as an allocation of its own does.
        Some((&f, pkt)) if f == format => Ok(pkt.to_vec()),
        _ => Err(MarshalError::new(Errno::EBADMSG)),
    }
}

/// The writes a file of `format`, `bytes`, holds, each record its leaf's
/// class's.
fn decode_writes(format: u8, bytes: &[u8]) -> Result<Vec<Write>, MarshalError> {
    let pkt = packet(format, bytes)?;
    let root = TreeReader::new(&pkt)?.read_branch()?;
    let (hi, lo, id) = (
        root.at(column::KEY_HI)?.get_array::<u64>()?,
        root.at(column::KEY_LO)?.get_array::<u64>()?,
        root.at(column::ID)?.get_array::<u32>()?,
    );
    let (text_rows, texts) = (
        root.at(column::TEXT_ROW)?.get_array::<u32>()?,
        root.at(column::TEXT)?.get_strs()?,
    );
    if hi.len() != lo.len() || hi.len() != id.len() || text_rows.len() != texts.len() as usize {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    let mut ext: Vec<Option<Ext>> = vec![None; id.len()];
    for (r, t) in text_rows.iter().zip(texts.iter()) {
        let r = *r as usize;
        // A record is its leaf's class's, or the file is not this layout.
        if id.get(r).and_then(|&v| class_of(v)) != Some(Class::marker_text) {
            return Err(MarshalError::new(Errno::EINVAL));
        }
        ext[r] = Some(Ext::MarkerText(t.to_string()));
    }
    Ok((0..id.len())
        .map(|r| Write {
            key: (u128::from(hi[r]) << 64) | u128::from(lo[r]),
            id: id[r],
            ext: ext[r].take(),
        })
        .collect())
}

/// The leaves `bytes`, the file of `shard`, holds, whole or none: every one
/// of its shard, a zoom the store holds and an id a leaf holds.
fn decode_shard(shard: Tile, bytes: &[u8]) -> Result<Vec<Write>, MarshalError> {
    let writes = decode_writes(format::OBJECT, bytes)?;
    let fits = |w: &Write| {
        is_key(w.key)
            && (CUBE_ZOOM..=OBJECT_FINEST).contains(&node_of::<3>(w.key).0)
            && shard_of(w.key) == shard
            && u16::try_from(w.id).is_ok()
    };
    if !writes.iter().all(fits) {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    Ok(writes)
}

/// The top's rows, `<shard, z, fold>` as host.json `top` lays them out,
/// whole or none: each shard's a tile at `SHARD_ZOOM`, its Z one of its
/// two, its share of its cube filled at most the whole.
fn decode_top(bytes: &[u8]) -> Result<Vec<(u64, i32, u32)>, MarshalError> {
    let pkt = packet(format::OBJECT_TOP, bytes)?;
    let root = TreeReader::new(&pkt)?.read_branch()?;
    let (shard, z, fold) = (
        root.at(top::SHARD)?.get_array::<u64>()?,
        root.at(top::Z)?.get_array::<i32>()?,
        root.at(top::FOLD)?.get_array::<u32>()?,
    );
    if shard.len() != z.len() || shard.len() != fold.len() {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    let fits = |r: usize| {
        if !is_key(u128::from(shard[r]) << 64) {
            return false;
        }
        let t = Tile::of_subject(shard[r]);
        t.zoom == SHARD_ZOOM
            && t.subject() == shard[r]
            && SHARD_Z.contains(&z[r])
            && overwrite::high(fold[r]) <= FULL
    };
    if !(0..shard.len()).all(fits) {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    Ok((0..shard.len())
        .map(|r| (shard[r], z[r], fold[r]))
        .collect())
}

/// A batch is sent a while after its first write, so a burst of writes
/// goes up as one; what is pending is flushed when the page hides.
const SAVE_DELAY_MS: f64 = 300.0;

/// A write the store took, to go up as an edit.
pub fn send(write: Write) {
    let st = state();
    st.unsent.push(write);
    if st.save_due_ms.is_none() {
        st.save_due_ms = Some(now() + SAVE_DELAY_MS);
    }
}

static SAVES: Global<Notify> = Global::new();

fn saves() -> &'static mut Notify {
    SAVES.get(Notify::new)
}

pub fn step() {
    match state().save_due_ms {
        Some(due) if now() >= due => saves().notify(),
        Some(due) => wake_at(due),
        None => {}
    }
}

/// Sends the writes pending once a batch is due.
pub async fn task() {
    loop {
        notified(saves).await;
        save();
    }
}

/// Sends what is pending now, as the page hides.
pub fn flush() {
    save();
}

/// Sends the writes since the last batch as one edit batch, where the turn
/// has a write left (sys/io/file.rs `write`); else it is due on the next
/// turn. A batch that does not encode is not sent.
fn save() {
    let st = state();
    if st.save_due_ms.take().is_none() || st.unsent.is_empty() {
        return;
    }
    let Ok(batch) = encode_writes(format::OBJECT_EDITS, &st.unsent) else {
        st.unsent.clear();
        return;
    };
    if !file::write(root::OBJECT, EDITS, batch) {
        st.save_due_ms = Some(now());
        request_world_tick();
        return;
    }
    st.unsent.clear();
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod test;
