// pxl-kernel/io/load.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the I/O ledger like a page, sharing
//! its credits, and stays wanted until it arrives.
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
//! Where an asset lives is the host's. A model is a file below the assets'
//! root (io/file.rs), opened, then read and parsed by `gltf_task` on the
//! turns' quota; an image is decoded where the device is and comes back as
//! the host's handle (LOAD_START, `image_arrived`) — the file's own pixels,
//! top row first, no color conversion and no premultiplied alpha. A consumer
//! polls: None while the read is out or the asset is broken.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::abi::host::root;
use crate::global::Global;
use crate::io::demand::Ask;
use crate::io::file::{self, Owner};
use crate::io::sched::{attempt_of, settle};
use crate::layer::decor::asset_store::decor_assets_arrived;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::change::{request_world_tick, world_edge};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::abi::handle::Slots;

/// What an asset is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
}

/// What a load names: a shipped asset, or the texture pack's sheet, which the
/// ingest writes beside the table rather than as a row of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Asset(Asset),
    Sheet,
}

impl Load {
    pub fn number(self) -> u32 {
        match self {
            Load::Asset(a) => a as u32,
            Load::Sheet => Asset::ALL.len() as u32,
        }
    }

    pub fn of(number: u32) -> Option<Load> {
        if number == Asset::ALL.len() as u32 {
            return Some(Load::Sheet);
        }
        u16::try_from(number).ok().and_then(Asset::of).map(Load::Asset)
    }
}

/// `Arrived` exists because the read's credit is released when the bytes
/// land (parsing is not I/O and would starve page reads), but the asset is
/// not usable yet. Without it, "not usable" read as "not asked for" and one
/// model was re-read 38 times, once a turn, for as long as its parse took.
enum Entry<T> {
    Pending,
    /// The file is open as this descriptor, this long, and nothing has read
    /// or parsed it yet.
    Arrived(u32, u32),
    Ready(T),
    Failed,
}

pub const LOADS: usize = Asset::ALL.len() + 1;

struct Loader {
    gltf: Vec<Option<Entry<Rc<Gltf>>>>,
    image: Vec<Option<Entry<u32>>>,
    /// Kept so saying the wants every turn allocates nothing.
    wants: Vec<(u64, Ask)>,
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        gltf: (0..LOADS).map(|_| None).collect(),
        image: (0..LOADS).map(|_| None).collect(),
        wants: Vec::new(),
    })
}

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

pub fn load_failed(load: Load) -> bool {
    let l = loader();
    let n = load.number() as usize;
    matches!(l.gltf[n], Some(Entry::Failed)) || matches!(l.image[n], Some(Entry::Failed))
}

/// Kind in the high word, since one asset can be wanted both as a model and as an image.
pub fn subject_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | load.number() as u64
}

pub fn of_subject(subject: u64) -> Option<(LoadKind, Load)> {
    let kind = if subject >> 32 == 0 { LoadKind::Gltf } else { LoadKind::Image };
    Load::of(subject as u32).map(|load| (kind, load))
}

pub fn load_wants() -> &'static [(u64, Ask)] {
    let l = loader();
    l.wants.clear();
    for n in 0..LOADS {
        let load = Load::of(n as u32).or_bug(Errno::ENOTRECOVERABLE);
        if matches!(l.gltf[n], Some(Entry::Pending)) {
            l.wants.push((subject_of(LoadKind::Gltf, load), Ask::PLAIN));
        }
        if matches!(l.image[n], Some(Entry::Pending)) {
            l.wants.push((subject_of(LoadKind::Image, load), Ask::PLAIN));
        }
    }
    &l.wants
}

/// The credit ends when the host reports; parsing afterwards does not hold it.
fn ended(key: u64, outcome: Outcome) -> Option<(LoadKind, Load)> {
    let what = attempt_of(key).and_then(|(_, subject)| of_subject(subject));
    settle(key, outcome, None, Failure::default());
    what
}

static ARRIVALS: Global<Notify> = Global::new();

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// Opens a model's file for the scheduler's attempt `key`.
pub fn open_model(key: u64, load: Load) {
    file::open(root::ASSET, &load.number().to_string(), Owner::Model { key });
}

/// Stops a model's read, if one is open: the asset is wanted again, as after
/// a read that did not arrive.
pub fn cancel_model(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| *o == Owner::Model { key }) else { return false };
    file::close(fd);
    load_rejected(key);
    true
}

/// The host's answer to a model's open: `gltf_task`'s to read and parse, or
/// a read that did not arrive.
pub fn model_opened(fd: u32, key: u64, res: i32) {
    if res < 0 {
        file::close(fd);
        load_rejected(key);
        return;
    }
    let Some((_, load)) = ended(key, Outcome::ok) else {
        file::close(fd);
        return;
    };
    loader().gltf[load.number() as usize] = Some(Entry::Arrived(fd, res as u32));
    arrivals().notify();
}

fn next_arrival() -> Option<(usize, u32, u32)> {
    let l = loader();
    (0..LOADS).find_map(|n| match l.gltf[n] {
        Some(Entry::Arrived(body, bytes)) => Some((n, body, bytes)),
        _ => None,
    })
}

pub async fn gltf_task() {
    loop {
        notified(arrivals).await;
        while let Some((n, fd, bytes)) = next_arrival() {
            yield_if_spent(world_edge).await;
            let mut buf = vec![0u8; bytes as usize];
            file::read(fd, &mut buf);
            file::close(fd);
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

/// The transport failed, not the asset: it is wanted again, and the ledger
/// decides the wait.
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
