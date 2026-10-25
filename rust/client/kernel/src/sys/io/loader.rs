// pxl-kernel/sys/io/loader.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! A read that did not arrive is retried, and one the heap refused room for
//! waits, as a tile's body does, until the heap gives some back (the request
//! scheduler's `Wait::Room`): its bytes are taken when the open answers, and
//! its descriptor closed there with its credit. Bytes that do not parse are
//! final, since asking again returns the same bytes. An image is its file's
//! bytes, handed to the renderer under the asset's number, which makes its
//! texture on whatever device there is (pxl-render `Gpu::image`), and let
//! go once that texture lands: a device lost reads the file again. io is
//! transport, and the device the renderer's.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use xpute_guest::sched::edge::{next_turn, yield_if_spent};

use crate::sys::io::demand::Ask;
use crate::sys::io::failure::Ended;
use crate::sys::io::file::{self, Opened, Owner};
use crate::sys::io::host_spec::{root, SPHERE};
use crate::sys::io::request::{attempt_of, settle, Credit};
use crate::sys::io::source;
use crate::sys::load::{Load, LoadKind, LOADS};
use crate::sys::sched::edge::{request_world_tick, world_edge};
use pxl_spec::bake::published::BaseLayer;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// `Arrived` exists because the read's credit is released when the bytes
/// land (parsing is not I/O and would starve tile reads), but the asset is
/// not usable yet. Without it, "not usable" read as "not asked for" and one
/// model was re-read 38 times, once a turn, for as long as its parse took.
enum Entry {
    Pending,
    /// Its bytes in its arrival's task, being taken in.
    Arrived,
    /// A parsed model.
    Model(Rc<Gltf>),
    /// An image's bytes, sized by its header, until its texture lands.
    Image(Vec<u8>),
    /// An image whose texture landed on the device there is, its bytes gone.
    Landed,
    Failed,
}

struct Loader {
    /// By kind, then by load: one asset can be wanted as both.
    entries: [Vec<Option<Entry>>; 2],
    /// Kept so saying the wants every turn allocates nothing.
    wants: Vec<(u64, Ask)>,
}

impl Loader {
    fn at(&mut self, kind: LoadKind, n: usize) -> &mut Option<Entry> {
        &mut self.entries[kind as usize][n]
    }
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        entries: [(); 2].map(|()| (0..LOADS).map(|_| None).collect()),
        wants: Vec::new(),
    })
}

pub fn load_gltf(load: Load) -> Option<Rc<Gltf>> {
    let entry = loader().at(LoadKind::Gltf, load.number() as usize);
    match entry {
        Some(Entry::Model(g)) => Some(g.clone()),
        Some(_) => None,
        None => {
            *entry = Some(Entry::Pending);
            None
        }
    }
}

/// An image as the renderer takes it: the asset's number and its file's
/// bytes, once read, and no bytes once its texture landed; none while it
/// loads.
pub fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    let entry = loader().at(LoadKind::Image, load.number() as usize);
    match entry {
        Some(Entry::Image(file)) => Some((load.number(), file.as_slice())),
        Some(Entry::Landed) => Some((load.number(), &[])),
        Some(_) => None,
        None => {
            *entry = Some(Entry::Pending);
            None
        }
    }
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
            if matches!(l.at(kind, n), Some(Entry::Pending)) {
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

/// A model and an image of one load are the same file; which it is read as
/// is its attempt's subject's (`opened`).
pub fn open_asset(credit: Credit, load: Load) {
    let owner = Owner::Load { key: credit.key() };
    // The sphere's number is no tile's: a subject is never 0.
    const _: () = assert!(pxl_spec::space::hierarchy::morton::bits::<2>(pxl_spec::space::layer::tile::LEAF_ZOOM) < 63);
    match load {
        Load::Sphere => file::open(credit, BaseLayer::earth as u32, u64::from(SPHERE), owner),
        _ => file::open(credit, root::ASSET, u64::from(load.number()), owner),
    };
}

pub fn cancel_asset(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| *o == Owner::Load { key }) else {
        return false;
    };
    file::close(fd);
    load_rejected(key, Ended::Aborted);
    true
}

/// The open answered: its bytes taken now, as a tile's body is
/// (sys/io/fetch.rs `opened`), and its descriptor closed with its credit;
/// where the heap refuses them room, the want waits for room.
pub fn opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        load_rejected(key, Ended::Failed(opened.failure()));
        return;
    };
    let Some(what) = attempt_of(key).and_then(|(_, subject)| of_subject(subject)) else {
        file::close(fd);
        settle(key, Ended::Ok, None);
        return;
    };
    // SAFETY: the bytes are filled from the next turn on, and the arrival
    // reads them only after it.
    match unsafe { file::read_whole(fd, n) } {
        Ok(bytes) => {
            settle(key, Ended::Ok, None);
            let (kind, load) = what;
            arrive(kind, load.number() as usize, bytes);
        }
        Err(refused) => settle(key, Ended::Refused(refused), None),
    }
}

/// An image's bytes, read: kept, where a header gives its size
/// (pxl-spec `asset::image_size`); a file no header sizes is the asset's
/// own failure, so it is final.
fn image_arrived(n: usize, file: Vec<u8>) {
    *loader().at(LoadKind::Image, n) = Some(if pxl_spec::asset::image_size(&file).is_some() { Entry::Image(file) } else { Entry::Failed });
    // A turn hands it on.
    request_world_tick();
}

/// The renderer took the texture of the image numbered `image`
/// (pxl-render `take_landed`): its file goes.
pub fn image_landed(image: u32) {
    if let Some(entry @ Some(Entry::Image(_))) = loader().entries[LoadKind::Image as usize].get_mut(image as usize) {
        *entry = Some(Entry::Landed);
    }
}

/// The device went, and every texture an image landed in: each is read
/// again when next asked for. One still in its bytes is handed again.
pub fn images_lost() {
    for entry in &mut loader().entries[LoadKind::Image as usize] {
        if matches!(entry, Some(Entry::Landed)) {
            *entry = None;
        }
    }
}

/// A file's bytes, taken in by its own task: no arrival waits on another's
/// parse or landing. Where the turn's quota is spent before the tasks run, as
/// it is while the frame outweighs its share, each task advances a step a
/// turn, so k arrivals advance k steps, not one.
fn arrive(kind: LoadKind, n: usize, bytes: Vec<u8>) {
    *loader().at(kind, n) = Some(Entry::Arrived);
    // The pool holds a task for each kind of every load (TASKS), and each
    // has one file in flight at most, so there is always room.
    let spawned = xpute_guest::sched::task::spawn(arrival(kind, n, bytes), crate::sys::sched::edge::TaskPriority::Arrival as u8);
    xpute_kit::ensure!(spawned, ENOSPC, n);
}

/// An asset file arrived, read whole: a model parsed, an image kept until
/// its texture lands.
async fn arrival(kind: LoadKind, n: usize, buf: Vec<u8>) {
    // In once the host has run the turn's records.
    next_turn(world_edge).await;
    yield_if_spent(world_edge).await;
    if kind == LoadKind::Image {
        image_arrived(n, buf);
        return;
    }
    let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
        Ok(g) => Entry::Model(Rc::new(g)),
        Err(_) => Entry::Failed,
    };
    *loader().at(LoadKind::Gltf, n) = Some(entry);
    request_world_tick();
}

/// The transport failed, not the asset: it is wanted again, and the request scheduler
/// decides the wait.
pub fn load_rejected(key: u64, how: Ended) {
    let Some((kind, load)) = ended(key, how) else { return };
    *loader().at(kind, load.number() as usize) = None;
}
