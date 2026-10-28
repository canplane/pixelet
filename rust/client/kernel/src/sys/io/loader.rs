// pxl-kernel/sys/io/loader.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! Which file an asset references is the catalog's, the first file asked
//! for (`catalog`): a slot names no file until it lands, which its reader
//! takes as one still loading.
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
use crate::sys::io::host_spec::{root, CATALOG, SPHERE};
use crate::sys::io::request::{attempt_of, settle, Credit};
use crate::sys::io::source;
use crate::sys::load::{Load, LoadKind};
use crate::sys::sched::edge::{request_world_tick, world_edge};
use pxl_spec::asset::catalog::{Catalog, Slot};
use pxl_spec::bake::published::BaseLayer;
use xpute_guest::global::Global;

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
    /// Taken in where its reader keeps it, its bytes gone: an image's
    /// texture on the device there is, the catalog as `catalog` reads it.
    Landed,
    Failed,
}

struct Loader {
    /// By kind, then by number below the asset root, grown to the highest
    /// asked: one file can be wanted as several.
    entries: [Vec<Option<Entry>>; 3],
    /// The sphere's, by kind.
    sphere: [Option<Entry>; 3],
    /// Kept so saying the wants every turn allocates nothing.
    wants: Vec<(u64, Ask)>,
}

impl Loader {
    fn at(&mut self, kind: LoadKind, load: Load) -> &mut Option<Entry> {
        let n = match load {
            Load::Sphere => return &mut self.sphere[kind as usize],
            Load::File(n) => n as usize,
        };
        let row = &mut self.entries[kind as usize];
        if row.len() <= n {
            row.resize_with(n + 1, || None);
        }
        &mut row[n]
    }
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        entries: [Vec::new(), Vec::new(), Vec::new()],
        sphere: [None, None, None],
        wants: Vec::new(),
    })
}

static READ: Global<Option<Catalog>> = Global::new();

/// The catalog the host plugs in (pxl-spec asset/catalog.rs), asked for
/// when first read; none while it loads.
pub fn catalog() -> Option<&'static Catalog> {
    if let Some(c) = READ.get(|| None).as_ref() {
        return Some(c);
    }
    let entry = loader().at(LoadKind::Catalog, Load::File(CATALOG));
    if entry.is_none() {
        *entry = Some(Entry::Pending);
    }
    None
}

/// The file `slot` names; none while the catalog loads.
pub fn load_of(slot: Slot) -> Option<Load> {
    catalog().map(|c| Load::File(c.file(slot)))
}

pub fn load_gltf(load: Load) -> Option<Rc<Gltf>> {
    let entry = loader().at(LoadKind::Gltf, load);
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
    let entry = loader().at(LoadKind::Image, load);
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

/// Kind in the high word, since one file can be wanted as more than one.
pub fn subject_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | load.number() as u64
}

pub fn of_subject(subject: u64) -> Option<(LoadKind, Load)> {
    let kind = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog]
        .into_iter()
        .find(|k| *k as u64 == subject >> 32)?;
    Load::of(subject as u32).map(|load| (kind, load))
}

pub fn load_wants() -> &'static [(u64, Ask)] {
    let l = loader();
    l.wants.clear();
    // By subject, as a stream's wants are said: the models, the images,
    // then the catalog, each the files by number and the sphere past them.
    for kind in [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog] {
        let files = (0..l.entries[kind as usize].len() as u32).map(Load::File);
        for load in files.chain([Load::Sphere]) {
            // The Earth's is opened under the version its source declares.
            if load == Load::Sphere && source::top().is_none() {
                continue;
            }
            if matches!(l.at(kind, load), Some(Entry::Pending)) {
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
/// is its attempt's subject's (`on_opened`).
pub fn open_asset(credit: Credit, load: Load) {
    let owner = Owner::Load { key: credit.key() };
    // The sphere's number is no tile's: a subject is never 0.
    const _: () = assert!(
        pxl_spec::space::hierarchy::morton::bits::<2>(pxl_spec::space::layer::tile::LEAF_ZOOM) < 63
    );
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
    on_load_rejected(key, Ended::Aborted);
    true
}

/// The open answered: its bytes taken now, as a tile's body is
/// (sys/io/fetch.rs `on_opened`), and its descriptor closed with its credit;
/// where the heap refuses them room, the want waits for room.
pub fn on_opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        on_load_rejected(key, Ended::Failed(opened.failure()));
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
            arrive(kind, load, bytes);
        }
        Err(refused) => settle(key, Ended::Refused(refused), None),
    }
}

/// An image's bytes, read: kept, where a header gives its size
/// (pxl-spec `asset::image_size`); a file no header sizes is the asset's
/// own failure, so it is final.
fn on_image_arrived(load: Load, file: Vec<u8>) {
    *loader().at(LoadKind::Image, load) = Some(if pxl_spec::asset::image_size(&file).is_some() {
        Entry::Image(file)
    } else {
        Entry::Failed
    });
    // A turn hands it on.
    request_world_tick();
}

/// The catalog's bytes, read: kept where they read as the compiled slots,
/// else its failure, final, since asking again returns the same bytes.
fn on_catalog_arrived(load: Load, file: &[u8]) {
    let read = Catalog::read(file);
    *loader().at(LoadKind::Catalog, load) = Some(if read.is_some() {
        Entry::Landed
    } else {
        Entry::Failed
    });
    READ.set(read);
    request_world_tick();
}

/// The renderer took the texture of the image numbered `image`
/// (pxl-render `take_landed`): its file goes.
pub fn on_image_landed(image: u32) {
    let Some(load) = Load::of(image) else { return };
    let entry = loader().at(LoadKind::Image, load);
    if matches!(entry, Some(Entry::Image(_))) {
        *entry = Some(Entry::Landed);
    }
}

/// The device went, and every texture an image landed in: each is read
/// again when next asked for. One still in its bytes is handed again.
pub fn on_images_lost() {
    let l = loader();
    for entry in l.entries[LoadKind::Image as usize]
        .iter_mut()
        .chain([&mut l.sphere[LoadKind::Image as usize]])
    {
        if matches!(entry, Some(Entry::Landed)) {
            *entry = None;
        }
    }
}

/// A file's bytes, taken in by its own task: no arrival waits on another's
/// parse or landing. Where the turn's quota is spent before the tasks run, as
/// it is while the frame outweighs its share, each task advances a step a
/// turn, so k arrivals advance k steps, not one.
fn arrive(kind: LoadKind, load: Load, bytes: Vec<u8>) {
    *loader().at(kind, load) = Some(Entry::Arrived);
    // The pool holds a task for each slot, the catalog and the sphere
    // (TASKS): a slot is wanted as one kind of one file, and each has one
    // file in flight at most, so there is always room.
    let spawned = xpute_guest::sched::task::spawn(
        arrival(kind, load, bytes),
        crate::sys::sched::edge::TaskPriority::Arrival as u8,
    );
    xpute_kit::ensure!(spawned, ENOSPC, load.number());
}

/// An asset file arrived, read whole: a model parsed, an image kept until
/// its texture lands, the catalog read.
async fn arrival(kind: LoadKind, load: Load, buf: Vec<u8>) {
    // In once the host has run the turn's records.
    next_turn(world_edge).await;
    yield_if_spent(world_edge).await;
    match kind {
        LoadKind::Image => return on_image_arrived(load, buf),
        LoadKind::Catalog => return on_catalog_arrived(load, &buf),
        LoadKind::Gltf => {}
    }
    let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
        Ok(g) => Entry::Model(Rc::new(g)),
        Err(_) => Entry::Failed,
    };
    *loader().at(LoadKind::Gltf, load) = Some(entry);
    request_world_tick();
}

/// The transport failed, not the asset: it is wanted again, and the request scheduler
/// decides the wait.
pub fn on_load_rejected(key: u64, how: Ended) {
    let Some((kind, load)) = ended(key, how) else {
        return;
    };
    *loader().at(kind, load) = None;
}
