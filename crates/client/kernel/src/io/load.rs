// pxl-kernel/io/load.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! A read that did not arrive is retried; bytes that do not parse are final,
//! since asking again returns the same bytes. Images are decoded by the host
//! under a kernel-issued handle and never cross into the kernel.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use pxl_spec::asset::ingested::Asset;
use xpute_guest::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::abi::host::root;
use crate::io::demand::Ask;
use crate::io::failure::Ended;
use crate::io::file::{self, Opened, Owner};
use crate::io::request::{attempt_of, settle, Credit};
use crate::io::source;
use crate::sched::edge::{request_world_tick, world_edge};
use pxl_spec::bake::published::BaseLayer;
use pxl_spec::route::{tile_object, TILE_SPHERE};
use xpute_guest::abi::handle::Slots;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf = 0,
    Image = 1,
}

/// The texture pack's image is written beside the asset table, not as a row of
/// it; the Earth's picture whole is the earth layer's, under its version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Asset(Asset),
    Pack,
    Sphere,
}

impl Load {
    pub fn number(self) -> u32 {
        match self {
            Load::Asset(a) => a as u32,
            Load::Pack => Asset::ALL.len() as u32,
            Load::Sphere => Asset::ALL.len() as u32 + 1,
        }
    }

    pub fn of(number: u32) -> Option<Load> {
        match number.checked_sub(Asset::ALL.len() as u32) {
            Some(0) => Some(Load::Pack),
            Some(1) => Some(Load::Sphere),
            Some(_) => None,
            None => u16::try_from(number).ok().and_then(Asset::of).map(Load::Asset),
        }
    }
}

/// `Arrived` exists because the read's credit is released when the bytes
/// land (parsing is not I/O and would starve tile reads), but the asset is
/// not usable yet. Without it, "not usable" read as "not asked for" and one
/// model was re-read 38 times, once a turn, for as long as its parse took.
enum Entry<T> {
    Pending,
    /// Open as this descriptor, this long, unread.
    Arrived(u32, u32),
    /// An image read: the handle its texture is arriving under.
    Landing(u32),
    Ready(T),
    Failed,
}

/// Every asset, the pack and the sphere.
pub const LOADS: usize = Asset::ALL.len() + 2;

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
        // The Earth's is opened under the version its source declares.
        if load == Load::Sphere && source::top().is_none() {
            continue;
        }
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
fn ended(key: u64, ended: Ended) -> Option<(LoadKind, Load)> {
    let what = attempt_of(key).and_then(|(_, subject)| of_subject(subject));
    settle(key, ended, None);
    what
}

static ARRIVALS: Global<Notify> = Global::new();

static SETTLED: Global<bool> = Global::new();

/// Whether a load settled, arrived or failed, since last taken: what waits on
/// assets takes it on its turn rather than being called from here.
pub fn take_settled() -> bool {
    core::mem::take(SETTLED.get(|| false))
}

fn settled() {
    *SETTLED.get(|| false) = true;
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

pub fn open_asset(credit: Credit, kind: LoadKind, load: Load) {
    let key = credit.key();
    let owner = match kind {
        LoadKind::Gltf => Owner::Model { key },
        LoadKind::Image => Owner::Image { key },
    };
    match load {
        Load::Sphere => file::open(credit, BaseLayer::earth as u32, &tile_object(source::whole(BaseLayer::earth), TILE_SPHERE), owner),
        _ => file::open(credit, root::ASSET, &load.number().to_string(), owner),
    };
}

pub fn cancel_asset(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| *o == Owner::Model { key } || *o == Owner::Image { key }) else {
        return false;
    };
    file::close(fd);
    load_rejected(key, Ended::Aborted);
    true
}

static IMAGES: Global<Slots<(), { LOADS + 1 }>> = Global::new();

pub fn image_opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        load_rejected(key, Ended::Failed(opened.failure()));
        return;
    };
    let Some((_, load)) = ended(key, Ended::Ok) else {
        file::close(fd);
        return;
    };
    loader().image[load.number() as usize] = Some(Entry::Arrived(fd, n));
    arrivals().notify();
}

/// The GPU is gone, and with it every image the textures took: each is loaded
/// again, under a new handle, when it is next asked for.
pub fn images_lost() {
    let l = loader();
    for entry in &mut l.image {
        if let Some(Entry::Ready(h) | Entry::Landing(h)) = *entry {
            IMAGES.get(Slots::default).remove(h);
            *entry = None;
        }
    }
}

/// Whether image `n` is still the one arriving as `image`: the device's loss
/// forgets it.
fn arriving(n: usize, image: u32) -> bool {
    matches!(loader().image[n], Some(Entry::Landing(h)) if h == image)
}

/// An image's bytes, read, into a texture where the device is, and ready once
/// they are there. A file the device cannot decode is the asset's own
/// failure, so it is final.
async fn image_arrived(n: usize, file: Vec<u8>) {
    let image = IMAGES.get(Slots::default).insert(());
    loader().image[n] = Some(Entry::Landing(image));
    while pxl_render::gpu::api::device().is_none() {
        next_turn(world_edge).await;
        if !arriving(n, image) {
            return;
        }
    }
    if !pxl_render::gpu::api::image_arrive(image, file) {
        loader().image[n] = Some(Entry::Failed);
        settled();
        return;
    }
    while !pxl_render::gpu::api::image_landed(image) {
        next_turn(world_edge).await;
        if !arriving(n, image) {
            return;
        }
    }
    loader().image[n] = Some(Entry::Ready(image));
    settled();
    request_world_tick();
}

pub fn model_opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        load_rejected(key, Ended::Failed(opened.failure()));
        return;
    };
    let Some((_, load)) = ended(key, Ended::Ok) else {
        file::close(fd);
        return;
    };
    loader().gltf[load.number() as usize] = Some(Entry::Arrived(fd, n));
    arrivals().notify();
}

/// A file opened and not yet read: a model's or an image's, its descriptor
/// and length.
fn next_arrival() -> Option<(LoadKind, usize, u32, u32)> {
    let l = loader();
    let gltf = (0..LOADS).find_map(|n| match l.gltf[n] {
        Some(Entry::Arrived(fd, bytes)) => Some((LoadKind::Gltf, n, fd, bytes)),
        _ => None,
    });
    gltf.or_else(|| {
        (0..LOADS).find_map(|n| match l.image[n] {
            Some(Entry::Arrived(fd, bytes)) => Some((LoadKind::Image, n, fd, bytes)),
            _ => None,
        })
    })
}

/// Each asset file that arrives, read whole: a model parsed, an image put
/// into its texture.
pub async fn arrival_task() {
    loop {
        notified(arrivals).await;
        while let Some((kind, n, fd, bytes)) = next_arrival() {
            yield_if_spent(world_edge).await;
            let mut buf = Vec::new();
            if buf.try_reserve_exact(bytes as usize).is_err() {
                file::close(fd);
                match kind {
                    LoadKind::Gltf => loader().gltf[n] = Some(Entry::Failed),
                    LoadKind::Image => loader().image[n] = Some(Entry::Failed),
                }
                continue;
            }
            buf.resize(bytes as usize, 0);
            // SAFETY: `buf` is held across the turn the host fills it in.
            unsafe { file::read(fd, &mut buf) };
            file::close(fd);
            // In once the host has run the turn's records.
            next_turn(world_edge).await;
            if kind == LoadKind::Image {
                image_arrived(n, buf).await;
                continue;
            }
            let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
                Ok(g) => Entry::Ready(Rc::new(g)),
                Err(_) => Entry::Failed,
            };
            loader().gltf[n] = Some(entry);
            settled();
            request_world_tick();
        }
    }
}

/// The transport failed, not the asset: it is wanted again, and the request scheduler
/// decides the wait.
pub fn load_rejected(key: u64, how: Ended) {
    let Some((kind, load)) = ended(key, how) else { return };
    let l = loader();
    let n = load.number() as usize;
    match kind {
        LoadKind::Gltf => l.gltf[n] = None,
        LoadKind::Image => l.image[n] = None,
    }
    settled();
}
