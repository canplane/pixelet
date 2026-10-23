// pxl-kernel/io/load.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! A read that did not arrive is retried; bytes that do not parse are final,
//! since asking again returns the same bytes. An image is its file's bytes,
//! kept for as long as the kernel runs and handed to the renderer under the
//! asset's number, which makes its texture on whatever device there is
//! (pxl-render `Gpu::image`): io is transport, and the device the renderer's.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use xpute_guest::sched::edge::{next_turn, yield_if_spent};

use crate::abi::host::root;
use crate::asset::{Load, LoadKind, LOADS};
use crate::io::demand::Ask;
use crate::io::failure::Ended;
use crate::io::file::{self, Opened, Owner, Path};
use crate::io::request::{attempt_of, settle, Credit};
use crate::io::source;
use crate::sched::edge::{request_world_tick, world_edge};
use core::fmt::Write as _;
use pxl_spec::bake::published::BaseLayer;
use pxl_spec::route::{tile_object, TILE_SPHERE};
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// `Arrived` exists because the read's credit is released when the bytes
/// land (parsing is not I/O and would starve tile reads), but the asset is
/// not usable yet. Without it, "not usable" read as "not asked for" and one
/// model was re-read 38 times, once a turn, for as long as its parse took.
enum Entry<T> {
    Pending,
    /// Its file in its arrival's task, being read.
    Arrived,
    Ready(T),
    Failed,
}

struct Loader {
    gltf: Vec<Option<Entry<Rc<Gltf>>>>,
    image: Vec<Option<Entry<Vec<u8>>>>,
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

/// An image as the renderer takes it: the asset's number and its file's
/// bytes, once read; none while it loads.
pub fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    let entry = &mut loader().image[load.number() as usize];
    match entry {
        Some(Entry::Ready(file)) => Some((load.number(), file.as_slice())),
        Some(_) => None,
        None => {
            *entry = Some(Entry::Pending);
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
    // By subject, as a stream's wants are said: the models, then the images.
    for kind in [LoadKind::Gltf, LoadKind::Image] {
        for n in 0..LOADS {
            let load = Load::of(n as u32).or_bug(Errno::ENOTRECOVERABLE);
            // The Earth's is opened under the version its source declares.
            if load == Load::Sphere && source::top().is_none() {
                continue;
            }
            let pending = match kind {
                LoadKind::Gltf => matches!(l.gltf[n], Some(Entry::Pending)),
                LoadKind::Image => matches!(l.image[n], Some(Entry::Pending)),
            };
            if pending {
                l.wants.push((subject_of(kind, load), Ask::PLAIN));
            }
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

static SETTLED: Global<bool> = Global::new();

/// Whether a load settled, arrived or failed, since last taken: what waits on
/// assets takes it on its turn rather than being called from here.
pub fn take_settled() -> bool {
    core::mem::take(SETTLED.get(|| false))
}

fn settled() {
    *SETTLED.get(|| false) = true;
}

pub fn open_asset(credit: Credit, kind: LoadKind, load: Load) {
    let key = credit.key();
    let owner = match kind {
        LoadKind::Gltf => Owner::Model { key },
        LoadKind::Image => Owner::Image { key },
    };
    match load {
        Load::Sphere => file::open(
            credit,
            BaseLayer::earth as u32,
            Path::of(|w| tile_object(w, source::whole(BaseLayer::earth), TILE_SPHERE)).as_str(),
            owner,
        ),
        _ => file::open(credit, root::ASSET, Path::of(|w| write!(w, "{}", load.number())).as_str(), owner),
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

pub fn image_opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        load_rejected(key, Ended::Failed(opened.failure()));
        return;
    };
    let Some((_, load)) = ended(key, Ended::Ok) else {
        file::close(fd);
        return;
    };
    arrive(LoadKind::Image, load.number() as usize, fd, n);
}

/// An image's bytes, read: kept, where a header gives its size
/// (pxl-spec `asset::image_size`); a file no header sizes is the asset's
/// own failure, so it is final.
fn image_arrived(n: usize, file: Vec<u8>) {
    loader().image[n] = Some(if pxl_spec::asset::image_size(&file).is_some() { Entry::Ready(file) } else { Entry::Failed });
    settled();
    // A turn hands it on.
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
    arrive(LoadKind::Gltf, load.number() as usize, fd, n);
}

/// A file opened, read in its own task: no arrival waits on another's
/// parse or landing. Where the turn's quota is spent before the tasks run, as
/// it is while the frame outweighs its share, each task advances a step a
/// turn, so k arrivals advance k steps, not one.
fn arrive(kind: LoadKind, n: usize, fd: u32, bytes: u32) {
    match kind {
        LoadKind::Gltf => loader().gltf[n] = Some(Entry::Arrived),
        LoadKind::Image => loader().image[n] = Some(Entry::Arrived),
    }
    // The pool holds a task for each kind of every load (TASKS), and each
    // has one file in flight at most, so there is always room.
    let spawned = xpute_guest::sched::task::spawn(arrival(kind, n, fd, bytes), crate::sched::edge::TaskPriority::Arrival as u8);
    xpute_kit::ensure!(spawned, ENOSPC, n);
}

/// An asset file arrived, read whole: a model parsed, an image put into its
/// texture.
async fn arrival(kind: LoadKind, n: usize, fd: u32, bytes: u32) {
    yield_if_spent(world_edge).await;
    let mut buf = Vec::new();
    if buf.try_reserve_exact(bytes as usize).is_err() {
        file::close(fd);
        match kind {
            LoadKind::Gltf => loader().gltf[n] = Some(Entry::Failed),
            LoadKind::Image => loader().image[n] = Some(Entry::Failed),
        }
        return;
    }
    buf.resize(bytes as usize, 0);
    // SAFETY: `buf` is held across the turn the host fills it in.
    unsafe { file::read(fd, &mut buf) };
    file::close(fd);
    // In once the host has run the turn's records.
    next_turn(world_edge).await;
    if kind == LoadKind::Image {
        image_arrived(n, buf);
        return;
    }
    let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
        Ok(g) => Entry::Ready(Rc::new(g)),
        Err(_) => Entry::Failed,
    };
    loader().gltf[n] = Some(entry);
    settled();
    request_world_tick();
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
