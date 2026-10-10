// pxl-kernel/layer/object/repository.rs

//! Where placed objects are kept and come from, the one way the object
//! layer's records are read and written: what is asked for arrives and a
//! write is sent, each `<key, id>`. How they are kept is this file's alone;
//! for now every leaf is in the page's storage, all asked for at once.

use base64::Engine;
use pxl_spec::space::hierarchy::morton::key_of;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::sched::edge::{notified, Notify};

use crate::io::file::{self, Kept, KeptFile};
use crate::kernel;
use crate::layer::object::store::{apply, written, Write};
use crate::sched::edge::wake_at;

struct State {
    save_due_ms: Option<f64>,
    /// The saved world is in: until then nothing is written over it.
    loaded: bool,
}

static STATE: Global<State> = Global::new();

fn store_state() -> &'static mut State {
    STATE.get(|| State { save_due_ms: None, loaded: false })
}

// Kept as one blob of every leaf as the write that made it, `<key, id>`:
// [key_hi u64[], key_lo u64[], id u32[]]. The layout's version is in the
// storage key; a blob of another layout is never read.

fn encode() -> Result<String, MarshalError> {
    let (mut hi, mut lo, mut id) = (Vec::new(), Vec::new(), Vec::new());
    kernel().objects.each(&mut |e| {
        let key = key_of(e.zoom, e.at);
        hi.push((key >> 64) as u64);
        lo.push(key as u64);
        id.push(e.state);
    });
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        b.u64_array(Some(&hi)).u64_array(Some(&lo)).u32_array(Some(&id));
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

fn decode(blob: &str) -> Result<(), MarshalError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(blob).map_err(|_| MarshalError::new(Errno::EINVAL))?;
    let root = TreeReader::new(&bytes)?.read_branch()?;
    let (hi, lo, id) = (root.at(0)?.get_array::<u64>()?, root.at(1)?.get_array::<u64>()?, root.at(2)?.get_array::<u32>()?);
    if hi.len() != lo.len() || hi.len() != id.len() {
        return Err(MarshalError::new(Errno::EINVAL));
    }
    apply((0..id.len()).map(|r| Write {
        key: (u128::from(hi[r]) << 64) | u128::from(lo[r]),
        id: id[r],
    }));
    Ok(())
}

/// Asks for every tile.
pub fn open() {
    file::open_kept(Kept::Objects);
}

fn keep(blob: String) {
    file::write_kept(Kept::Objects, blob.into_bytes());
}

/// A missing blob starts the world empty. One that cannot be read or decoded
/// here — too large, an error, a layout this build does not know — leaves the
/// world unsaved this session, so it is never written over.
fn loaded() {
    let st = store_state();
    if st.loaded {
        return;
    }
    let blob = match file::take_kept(Kept::Objects) {
        None | Some(KeptFile::Unreadable) => return,
        Some(KeptFile::None) => {
            st.loaded = true;
            return;
        }
        Some(KeptFile::Bytes(blob)) => blob,
    };
    if core::str::from_utf8(&blob).map_or(true, |raw| decode(raw).is_err()) {
        // What decoded before the failure was already read; the blob stays unsaved over.
        let regions = kernel().objects.clear();
        if !regions.is_empty() {
            written(&regions);
        }
        return;
    }
    st.loaded = true;
}

/// A save encodes the whole store, so it is put off to merge a burst of
/// writes; what is pending is flushed when the page hides.
const SAVE_DELAY_MS: f64 = 300.0;

/// A write to be kept.
pub fn send(_: &Write) {
    let st = store_state();
    if st.save_due_ms.is_none() {
        st.save_due_ms = Some(now() + SAVE_DELAY_MS);
    }
}

static SAVES: Global<Notify> = Global::new();

fn saves() -> &'static mut Notify {
    SAVES.get(Notify::new)
}

pub fn step() {
    loaded();
    if !store_state().loaded {
        return;
    }
    match store_state().save_due_ms {
        Some(due) if now() >= due => saves().notify(),
        Some(due) => wake_at(due),
        None => {}
    }
}

/// Keeps the whole store once a save is due.
pub async fn task() {
    loop {
        notified(saves).await;
        if store_state().save_due_ms.take().is_none() {
            continue;
        }
        if let Ok(blob) = encode() {
            keep(blob);
        }
    }
}

/// Keeps what is pending now, as the page hides.
pub fn flush() {
    if !store_state().loaded || store_state().save_due_ms.take().is_none() {
        return;
    }
    if let Ok(blob) = encode() {
        keep(blob);
    }
}
