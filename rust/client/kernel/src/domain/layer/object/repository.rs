// pxl-kernel/domain/layer/object/repository.rs

//! Where placed objects are kept and come from, the one way the object
//! layer's records are read and written: what is asked for arrives and a
//! write is sent, each `<key, id>`. How they are kept is this file's alone.
//!
//! They are kept a shard at a time (pxl-spec-core `SHARD_ZOOM`), a file a
//! shard below OBJECT (host.json `root`) named by the shard's subject, as a
//! bake's tile is named by its own; a shard with none has no file. What
//! keeps the files is the host's (ts/client/src/driver/namespace/object.ts).
//! A shard is read as a bake's tile is, through the request scheduler
//! (sys/io/request.rs) and the fetches' arrival (sys/io/fetch.rs), wanted
//! where the view asks for it (`want`), and its leaves go into the layer's
//! one store (store.rs). A shard read stays read. A write marks its shard,
//! and each shard marked is written back whole.

use fnv::FnvHashSet;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{units, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{key_of, node_of};
use pxl_spec::space::hierarchy::tree::ancestor_of;
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
use crate::domain::layer::object::store::{apply, Write};
use crate::sys::io::demand::{Ask, Phases, Stream, TaskPhaseName};
use crate::sys::io::failure::Failure;
use crate::sys::io::fetch::{body, release};
use crate::sys::io::file;
use crate::sys::io::host_spec::{format, root};
use crate::sys::io::request::declare;
use crate::sys::sched::edge::{request_world_tick, wake_at};
use pxl_spec::object::ext::{class_of, Class};

/// The most shards wanted at once: those within the ground's demand at the
/// shard's zoom about the eye (domain/view/demand.rs `object_step`), as many
/// as a zoom's demand names of the ground's tiles (pxl-spec layer/demand.rs
/// `demand_tiles`).
pub const WANTS: u32 = GROUND_DEMAND_TILES;

#[derive(Default)]
struct State {
    /// The shards read, by subject: their leaves are in the store, or they
    /// have none.
    read: FnvHashSet<u64>,
    /// The shards a write changed since they were last written, by subject.
    unsaved: FnvHashSet<u64>,
    save_due_ms: Option<f64>,
    /// The shards `want` asks for, by subject; reused across turns.
    wants: Vec<(u64, Ask)>,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(State::default)
}

/// The shard the leaf `key` names lies in. Every leaf is a shard or finer
/// (store.rs `Store::write`).
fn shard_of(key: u128) -> Tile {
    let (zoom, at) = node_of::<3>(key);
    tile_at(SHARD_ZOOM, ancestor_of([at[0], at[1]], zoom - SHARD_ZOOM))
}

/// Whether every shard under `space` was read, so a write there stands
/// over what its file held (domain/query/limit.rs `editable`).
pub fn holds(space: &Aabb) -> bool {
    let side = units(SHARD_ZOOM);
    let span = |k: usize| {
        let first = (space.min[k] / side).floor() as i32;
        first..=((space.max[k] / side).ceil() as i32 - 1).max(first)
    };
    let read = &state().read;
    span(1).all(|y| span(0).all(|x| read.contains(&tile_at(SHARD_ZOOM, [x, y]).subject())))
}

/// The shards the view asks for (domain/view/demand.rs `object_step`), each
/// with its ask, no more than `WANTS`: those not yet read are wanted, in
/// place of what was wanted before.
pub fn want(asked: impl Iterator<Item = (Tile, Ask)>) {
    let st = state();
    st.wants.clear();
    for (shard, ask) in asked {
        if !st.read.contains(&shard.subject()) {
            st.wants.push((shard.subject(), ask));
        }
    }
    st.wants.sort_unstable_by_key(|&(subject, _)| subject);
    declare(Stream::object, &st.wants);
}

/// A shard's file arrived, or its absence: its leaves go into the store, and
/// the shard is read. One that does not decode leaves the store as it was
/// and the shard unread, so nothing is written over it.
pub fn on_arrived(shard: Tile, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    let at = now();
    let result = match block {
        None => Ok(()),
        Some(b) => {
            let decoded = decode(shard, body(b));
            release(Some(b));
            decoded.map_err(Failure::from)
        }
    };
    phases.push(TaskPhaseName::decode_ms, now() - at);
    if result.is_ok() {
        state().read.insert(shard.subject());
    }
    result
}

// A shard's file: its format byte (host.json `format.OBJECT`), then every leaf
// of the shard as the write that made it, `<key, id, ext>`: [key_hi u64[],
// key_lo u64[], id u32[]], then a class's records, each its leaf's row and its
// fields as columns: a marker's text [row u32[], text strs]. A file of another
// format is never read. A shard with no leaf is written empty, which the host
// keeps as no file.

fn encode(shard: Tile) -> Result<Vec<u8>, MarshalError> {
    let (mut hi, mut lo, mut id) = (Vec::new(), Vec::new(), Vec::new());
    let (mut text_rows, mut texts) = (Vec::new(), Vec::new());
    let object = object();
    object.entries_in(&shard.region(), &mut |e| {
        // A leaf beside the shard meets its edge.
        let key = key_of(e.zoom, e.at);
        if e.zoom < SHARD_ZOOM || shard_of(key) != shard {
            return;
        }
        match object.ext(e) {
            Some(Ext::MarkerText(t)) => {
                text_rows.push(id.len() as u32);
                texts.push(t.clone());
            }
            None => {}
        }
        hi.push((key >> 64) as u64);
        lo.push(key as u64);
        id.push(e.state);
    });
    if id.is_empty() {
        return Ok(Vec::new());
    }
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
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
    file.push(format::OBJECT);
    file.extend_from_slice(&pkt);
    Ok(file)
}

/// `bytes`, the file of `shard`, into the store whole, or none of it: every
/// leaf of its shard, a zoom the store holds and an id a leaf holds.
fn decode(shard: Tile, bytes: &[u8]) -> Result<(), MarshalError> {
    let Some((&format::OBJECT, pkt)) = bytes.split_first() else {
        return Err(MarshalError::new(Errno::EBADMSG));
    };
    // A packet starts on a word (`TreeReader`), as an allocation of its own does.
    let pkt = pkt.to_vec();
    let root = TreeReader::new(&pkt)?.read_branch()?;
    let (hi, lo, id) = (
        root.at(0)?.get_array::<u64>()?,
        root.at(1)?.get_array::<u64>()?,
        root.at(2)?.get_array::<u32>()?,
    );
    let (text_rows, texts) = (root.at(3)?.get_array::<u32>()?, root.at(4)?.get_strs()?);
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
    let writes: Vec<Write> = (0..id.len())
        .map(|r| Write {
            key: (u128::from(hi[r]) << 64) | u128::from(lo[r]),
            id: id[r],
            ext: ext[r].take(),
        })
        .collect();
    let fits = |w: &Write| {
        (CUBE_ZOOM..=OBJECT_FINEST).contains(&node_of::<3>(w.key).0)
            && shard_of(w.key) == shard
            && u16::try_from(w.id).is_ok()
    };
    if !writes.iter().all(fits) {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    apply(writes);
    Ok(())
}

/// A save encodes each shard written, so it is put off to merge a burst of
/// writes; what is pending is flushed when the page hides.
const SAVE_DELAY_MS: f64 = 300.0;

/// A write to be kept: its shard is written back.
pub fn send(write: &Write) {
    let st = state();
    st.unsaved.insert(shard_of(write.key).subject());
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

/// Keeps the shards written once a save is due.
pub async fn task() {
    loop {
        notified(saves).await;
        save();
    }
}

/// Keeps what is pending now, as the page hides.
pub fn flush() {
    save();
}

/// Writes each shard written since it was last written, as many as the turn
/// allows (sys/io/file.rs `write`); the rest are due on the next turn. A
/// shard that does not encode stays as its file holds it.
fn save() {
    let st = state();
    if st.save_due_ms.take().is_none() {
        return;
    }
    let mut unsaved: Vec<u64> = st.unsaved.iter().copied().collect();
    unsaved.sort_unstable();
    for subject in unsaved {
        let Ok(file) = encode(Tile::of_subject(subject)) else {
            st.unsaved.remove(&subject);
            continue;
        };
        if !file::write(root::OBJECT, subject, file) {
            st.save_due_ms = Some(now());
            request_world_tick();
            return;
        }
        st.unsaved.remove(&subject);
    }
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod test;
