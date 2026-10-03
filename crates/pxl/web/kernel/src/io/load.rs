// pxl-kernel/io/load.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the I/O ledger like a page, sharing
//! its credits, and stays wanted until it arrives.
//!
//! A read that did not arrive is retried; bytes that do not parse are final,
//! since asking again returns the same bytes. Images are decoded by the host
//! under a kernel-issued handle and never cross into the kernel.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::abi::host::{op, root};
use crate::host::host_record;
use crate::io::demand::Ask;
use crate::io::file::{self, Owner};
use crate::io::sched::{attempt_of, settle};
use crate::layer::decor::asset_store::decor_assets_arrived;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::change::{request_world_tick, world_edge};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::abi::handle::Slots;
use xpute_runtime::global::Global;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
}

/// The texture pack's sheet is written beside the asset table, not as a row of it.
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
    /// A model: open as this descriptor, this long, unparsed. An image: the
    /// handle it is being decoded under.
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

pub fn open_asset(key: u64, kind: LoadKind, load: Load) {
    let owner = match kind {
        LoadKind::Gltf => Owner::Model { key },
        LoadKind::Image => Owner::Image { key },
    };
    file::open(root::ASSET, &load.number().to_string(), owner);
}

pub fn cancel_asset(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| *o == Owner::Model { key } || *o == Owner::Image { key }) else {
        return false;
    };
    file::close(fd);
    load_rejected(key);
    true
}

static IMAGES: Global<Slots<()>> = Global::new();

/// The file is closed in the same turn: the host takes its bytes when it runs the decode.
pub fn image_opened(fd: u32, key: u64, res: i32) {
    if res < 0 {
        file::close(fd);
        load_rejected(key);
        return;
    }
    let Some((_, load)) = ended(key, Outcome::ok) else {
        file::close(fd);
        return;
    };
    let image = IMAGES.get(Slots::default).insert(());
    host_record(op::IMAGE_DECODE, &[fd, image], &[]);
    file::close(fd);
    loader().image[load.number() as usize] = Some(Entry::Arrived(image, 0));
}

/// A file the device cannot decode is the asset's own failure, so it is final.
pub fn image_decoded(image: u32, res: i32) {
    let l = loader();
    let Some(n) = (0..LOADS).find(|&n| matches!(l.image[n], Some(Entry::Arrived(h, _)) if h == image)) else {
        return;
    };
    l.image[n] = Some(if res == 0 { Entry::Ready(image) } else { Entry::Failed });
    decor_assets_arrived();
    request_world_tick();
}

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
