// pxl-kernel/io/load.rs

//! Shared loading and caching of the models and the skins, so any consumer —
//! the companions, the decor kinds, the picker's preview — hits the network and
//! the parser at most once per asset.
//!
//! An asset asked for here for the first time becomes a want like any other
//! (`load_wants`, `io/sched.rs`): it shares the credits with the pages, so a
//! hundred and twenty-eight of them do not all go at once, and it waits its
//! turn among them rather than beside them.
//!
//! It is never stale. A page is wanted while the view is where it is; an
//! asset is wanted from the first time something needs it until it is there,
//! so nothing withdraws one and `Pending` is the whole of the record that it
//! is wanted.
//!
//! **Two failures, and only one of them is worth trying again.** A read that
//! did not arrive is the transport's, and the asset goes back to being wanted
//! — the scheduler holds it for a while first. Bytes that do not parse are
//! the asset's own, and asking for them again brings back the same bytes, so
//! that one is final.
//!
//! The reading is the shell's, and so is where an asset lives: it answers
//! with the bytes of a model it holds (`gltf_arrived`, then read and parsed by
//! `gltf_task` on the turns' quota) or the handle of an image it
//! decoded and holds (`image_arrived`) — the file's own pixels, top row
//! first, no color conversion and no premultiplied alpha. A consumer polls:
//! None while the read is out or the asset is broken.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{next_turn, notified, yield_if_spent, Notify};

pub use crate::abi::cmd::LoadKind;
use crate::global::Global;
use crate::host::body_read;
use crate::io::policy;
use crate::io::sched::{attempt_of, settle, Rank};
use crate::layer::decor::asset_store::decor_assets_arrived;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::change::{request_world_tick, world_edge};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// What a load names: a shipped asset, or the texture pack's sheet, which the
/// ingest writes beside the table rather than as a row of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Asset(Asset),
    Sheet,
}

impl Load {
    /// Its number on the wire (LOAD_START): an asset's own, the sheet one past
    /// the last.
    pub fn number(self) -> u32 {
        match self {
            Load::Asset(a) => a as u32,
            Load::Sheet => Asset::ALL.len() as u32,
        }
    }

    /// The load a number names, or none.
    pub fn of(number: u32) -> Option<Load> {
        if number == Asset::ALL.len() as u32 {
            return Some(Load::Sheet);
        }
        u16::try_from(number).ok().and_then(Asset::of).map(Load::Asset)
    }
}

/// Where an asset is between being wanted and being usable.
///
/// `Arrived` is the state this had to grow, and the bug it was grown for: the
/// read's credit is released the moment the bytes land, because parsing is not
/// I/O and a parse that spans turns would starve the page reads — but the
/// asset is not usable yet, and anything that reads "not usable" as "not asked
/// for" asks again. One model was read thirty-eight times, once a turn, for as
/// long as its parse took, each read holding another block of the same bytes.
///
/// So the credit ends at the network and the entry does not. It is the state
/// Cesium calls `PROCESSING` — "request received, contents are being processed
/// for rendering" — between its `LOADING` and `READY`, and the reason Bevy's
/// one `Loading` runs until an asset is *in the World* rather than until its
/// bytes arrive. Bevy can spend one state on both because it has no credits to
/// free in the middle; this has, so it takes two.
enum Entry<T> {
    /// Wanted, and the read for it is the scheduler's business.
    Pending,
    /// The shell holds the bytes at this body, this long, and nothing has
    /// parsed them yet.
    Arrived(u32, u32),
    Ready(T),
    Failed,
}

/// Loads by number: an asset's own, the sheet one past the last.
const LOADS: usize = Asset::ALL.len() + 1;

struct Loader {
    gltf: Vec<Option<Entry<Rc<Gltf>>>>,
    image: Vec<Option<Entry<u32>>>,
    /// What `load_wants` last said, kept so saying it again every turn asks
    /// the heap for nothing.
    wants: Vec<(u64, Rank)>,
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        gltf: (0..LOADS).map(|_| None).collect(),
        image: (0..LOADS).map(|_| None).collect(),
        wants: Vec::new(),
    })
}

/// A parsed glTF, once it has arrived.
pub fn load_gltf(load: Load) -> Option<Rc<Gltf>> {
    let l = loader();
    match &l.gltf[load.number() as usize] {
        Some(Entry::Ready(g)) => Some(g.clone()),
        Some(_) => None,
        None => {
            l.gltf[load.number() as usize] = Some(Entry::Pending);
            None
        }
    }
}

/// An image, as the shell's handle, once it has arrived.
pub fn load_image(load: Load) -> Option<u32> {
    let l = loader();
    match &l.image[load.number() as usize] {
        Some(Entry::Ready(h)) => Some(*h),
        Some(_) => None,
        None => {
            l.image[load.number() as usize] = Some(Entry::Pending);
            None
        }
    }
}

/// Whether a load has failed: it is never asked for again.
pub fn load_failed(load: Load) -> bool {
    let l = loader();
    let n = load.number() as usize;
    matches!(l.gltf[n], Some(Entry::Failed)) || matches!(l.image[n], Some(Entry::Failed))
}

/// What a want names: the kind in the high word and the load's number in the
/// low one, since one asset can be wanted as both a model and an image.
pub fn subject_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | load.number() as u64
}

pub fn of_subject(subject: u64) -> Option<(LoadKind, Load)> {
    let kind = if subject >> 32 == 0 { LoadKind::Gltf } else { LoadKind::Image };
    Load::of(subject as u32).map(|load| (kind, load))
}

/// Every asset something has asked for and has not got. Said in full every
/// turn: a set that is the same set changes nothing.
pub fn load_wants() -> &'static [(u64, Rank)] {
    let l = loader();
    l.wants.clear();
    for n in 0..LOADS {
        let load = Load::of(n as u32).or_bug(Errno::ENOTRECOVERABLE);
        if matches!(l.gltf[n], Some(Entry::Pending)) {
            l.wants.push((subject_of(LoadKind::Gltf, load), policy::asset()));
        }
        if matches!(l.image[n], Some(Entry::Pending)) {
            l.wants.push((subject_of(LoadKind::Image, load), policy::asset()));
        }
    }
    &l.wants
}

/// Ends the attempt a key names and says what it was for.
///
/// The read is over the moment the host reports it. Parsing what it brought
/// is the kernel's own work afterward and does not hold the credit — which
/// is the difference between a read and what is done with it.
fn ended(key: u64, outcome: Outcome) -> Option<(LoadKind, Load)> {
    let what = attempt_of(key).and_then(|(_, subject)| of_subject(subject));
    settle(key, outcome, None, Failure::default(), policy::retry_after(outcome));
    what
}

static ARRIVALS: Global<Notify> = Global::new();

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// The host holds a model's bytes as `body`: `gltf_task`'s to parse.
pub fn gltf_arrived(key: u64, body: u32, bytes: u32) {
    let Some((_, load)) = ended(key, Outcome::ok) else { return };
    loader().gltf[load.number() as usize] = Some(Entry::Arrived(body, bytes));
    arrivals().notify();
}

/// The first model whose bytes are in and unparsed.
fn next_arrival() -> Option<(usize, u32, u32)> {
    let l = loader();
    (0..LOADS).find_map(|n| match l.gltf[n] {
        Some(Entry::Arrived(body, bytes)) => Some((n, body, bytes)),
        _ => None,
    })
}

/// The models' task: each arrival read and parsed on the turns' quota, the
/// parse pausing between its units.
pub async fn gltf_task() {
    loop {
        notified(arrivals).await;
        while let Some((n, body, bytes)) = next_arrival() {
            yield_if_spent(world_edge).await;
            let mut buf = vec![0u8; bytes as usize];
            body_read(body, &mut buf);
            // In once the host has run the turn's records.
            next_turn(world_edge).await;
            let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
                Ok(g) => Entry::Ready(Rc::new(g)),
                Err(_) => Entry::Failed,
            };
            loader().gltf[n] = Some(entry);
            decor_assets_arrived();
            request_world_tick();
        }
    }
}

/// The shell's handle for a decoded image.
pub fn image_arrived(key: u64, handle: u32) {
    let Some((_, load)) = ended(key, Outcome::ok) else { return };
    loader().image[load.number() as usize] = Some(Entry::Ready(handle));
    decor_assets_arrived();
    request_world_tick();
}

/// The host's report that a read did not arrive. The asset goes back to
/// being wanted — the transport failed, not the asset — and the scheduler
/// decides how long it waits before the next attempt.
pub fn load_rejected(key: u64) {
    let Some((kind, load)) = ended(key, Outcome::error) else { return };
    let l = loader();
    let n = load.number() as usize;
    match kind {
        LoadKind::Gltf => l.gltf[n] = None,
        LoadKind::Image => l.image[n] = None,
    }
    decor_assets_arrived();
}
