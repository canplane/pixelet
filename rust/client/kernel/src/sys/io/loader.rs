// pxl-kernel/sys/io/loader.rs

//! Shared loading of models and images, so each asset is fetched at most
//! once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! Which file an asset references is the catalog's, the first file asked
//! for (`catalog`): a slot names no file until it lands, which its reader
//! takes as one still loading.
//!
//! A read that did not arrive is retried, and one the heap refused room for
//! waits, as a tile's body does, until the heap gives some back (the request
//! scheduler's `Wait::Room`): its room is taken when the open answers, and
//! its descriptor closed there with its credit. Bytes that do not parse are
//! final, since asking again returns the same bytes. A model is its file's
//! bytes, handed over once to the scene, which parses them and keeps the
//! parse (`take_gltf`). An image is its file's bytes, handed to the renderer
//! under the asset's number, which makes its texture on whatever device
//! there is (pxl-render `Gpu::image`), and let go once that texture lands: a
//! device lost reads the file again. io is transport: the model the scene's,
//! and the device the renderer's.

use xpute_core::sched::edge::{runtime_turn, yield_if_spent};

use crate::sys::io::demand::Order;
use crate::sys::io::failure::{Ended, Failure};
use crate::sys::io::file::{self, Opened, Owner, Reading};
use crate::sys::io::file_spec::{root, CATALOG};
use crate::sys::io::request::{attempt_of, settle, Credit};
use crate::sys::io::source;
use crate::sys::load::{Load, LoadKind};
use crate::sys::sched::edge::TaskPriority;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use pxl_spec::asset;
use pxl_spec::asset::catalog::{Catalog, Slot};
use pxl_spec::bake::publication::BaseLayer;
use xpute_core::global::Global;
use xpute_core::sched::task;

/// `Arrived` exists because the read's credit is released when the bytes
/// land, and the asset is not usable until its task has taken them in.
enum Entry {
    Pending,
    /// Its bytes in its arrival's task, being taken in.
    Arrived,
    /// Its file's bytes, until its reader takes them in, as its kind's
    /// table says: a model's until the scene takes them (`take_gltf`), an
    /// image's, sized by its header, until its texture lands.
    Bytes(Vec<u8>),
    /// Taken in where its reader keeps it, its bytes gone: a model as the
    /// scene parsed it, an image's texture on the device there is, the
    /// catalog as `catalog` reads it.
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
    wants: Vec<(u64, Order)>,
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

/// The catalog the client plugs in (pxl-spec asset/catalog.rs), asked for
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

/// A model's file's bytes, once read, handed over once: none while it
/// loads, and once taken.
pub fn take_gltf(load: Load) -> Option<Vec<u8>> {
    let entry = loader().at(LoadKind::Gltf, load);
    match entry {
        Some(Entry::Bytes(_)) => match entry.replace(Entry::Landed) {
            Some(Entry::Bytes(file)) => Some(file),
            _ => None,
        },
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
        Some(Entry::Bytes(file)) => Some((load.id(), file.as_slice())),
        Some(Entry::Landed) => Some((load.id(), &[])),
        Some(_) => None,
        None => {
            *entry = Some(Entry::Pending);
            None
        }
    }
}

/// Kind in the high word, since one file can be wanted as more than one.
pub fn id_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | load.id() as u64
}

pub fn of_id(id: u64) -> Option<(LoadKind, Load)> {
    let kind = [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog]
        .into_iter()
        .find(|k| *k as u64 == id >> 32)?;
    Load::of(id as u32).map(|load| (kind, load))
}

pub fn load_wants() -> &'static [(u64, Order)] {
    let l = loader();
    l.wants.clear();
    // By id, as a stream's wants are said: the models, the images,
    // then the catalog, each the files by number and the sphere past them.
    for kind in [LoadKind::Gltf, LoadKind::Image, LoadKind::Catalog] {
        let files = (0..l.entries[kind as usize].len() as u32).map(Load::File);
        for load in files.chain([Load::Sphere]) {
            // The Earth's is opened under the version its source declares.
            if load == Load::Sphere && source::top().is_none() {
                continue;
            }
            if matches!(l.at(kind, load), Some(Entry::Pending)) {
                l.wants.push((id_of(kind, load), Order::LAST));
            }
        }
    }
    &l.wants
}

/// The credit ends when the runtime reports; taking the bytes in afterwards
/// does not hold it.
fn on_ended(key: u64, ended: Ended) -> Option<(LoadKind, Load)> {
    let what = attempt_of(key).and_then(|(_, id)| of_id(id));
    settle(key, ended, None);
    what
}

/// A model and an image of one load are the same file; which it is read as
/// is its attempt's id's (`on_opened`).
pub fn open_asset(credit: Credit, load: Load) {
    let owner = Owner::Load { key: credit.key() };
    let root = match load {
        Load::Sphere => BaseLayer::earth as u32,
        Load::File(_) => root::ASSET,
    };
    file::open(credit, root, load.path(), owner);
}

pub fn cancel_asset(key: u64) -> bool {
    let Some(fd) = file::find(|o| *o == Owner::Load { key }) else {
        return false;
    };
    file::close(fd);
    on_load_rejected(key, Ended::Aborted);
    true
}

/// The open answered: its room taken now, as a tile's body's is
/// (sys/io/fetch.rs `on_opened`), and its descriptor closed with its credit;
/// where the heap refuses it, the want waits for room.
pub fn on_opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        on_load_rejected(key, Ended::Failed(Failure::from(opened)));
        return;
    };
    let Some(what) = attempt_of(key).and_then(|(_, id)| of_id(id)) else {
        file::close(fd);
        settle(key, Ended::Ok, None);
        return;
    };
    match file::read_whole(fd, n) {
        Ok(reading) => {
            settle(key, Ended::Ok, None);
            let (kind, load) = what;
            arrive(kind, load, reading);
        }
        Err(refused) => settle(key, Ended::Refused(refused), None),
    }
}

/// An image's bytes, read: kept, where a header gives its size
/// (pxl-spec `asset::image_size`); a file no header sizes is the asset's
/// own failure, so it is final.
fn on_image_arrived(load: Load, file: Vec<u8>) {
    *loader().at(LoadKind::Image, load) = Some(if asset::image_size(&file).is_some() {
        Entry::Bytes(file)
    } else {
        Entry::Failed
    });
    // A turn hands it on.
    request_world_tick();
}

/// A model's bytes, read: kept until they are taken.
fn on_gltf_arrived(load: Load, file: Vec<u8>) {
    *loader().at(LoadKind::Gltf, load) = Some(Entry::Bytes(file));
    request_world_tick();
}

/// The catalog's bytes, read: kept where they read as the compiled slots,
/// else its failure, final.
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
    if matches!(entry, Some(Entry::Bytes(_))) {
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
/// landing. Where the turn's deadline has passed before the tasks run, as
/// it is while the frame outweighs its share, each task advances a step a
/// turn, so k arrivals advance k steps, not one.
fn arrive(kind: LoadKind, load: Load, reading: Reading) {
    *loader().at(kind, load) = Some(Entry::Arrived);
    // The pool holds a task for each file in flight (sys/load.rs
    // `LOAD_FILES`): a slot is wanted as one kind of one file, and each has one
    // file in flight at most, so there is always room.
    let spawned = task::spawn(arrival(kind, load, reading), TaskPriority::Arrival as u8);
    xpute_kit::ensure!(spawned, ENOSPC, load.id());
}

/// An asset file arrived, read whole: a model's bytes kept until taken, an
/// image's until its texture lands, the catalog read, each once the runtime's head has passed
/// its read.
async fn arrival(kind: LoadKind, load: Load, mut reading: Reading) {
    let buf = loop {
        match file::take_read(reading) {
            Ok(buf) => break buf,
            Err(unread) => reading = unread,
        }
        runtime_turn(world_edge).await;
    };
    yield_if_spent(world_edge).await;
    match kind {
        LoadKind::Gltf => on_gltf_arrived(load, buf),
        LoadKind::Image => on_image_arrived(load, buf),
        LoadKind::Catalog => on_catalog_arrived(load, &buf),
    }
}

/// The transport failed, not the asset: it is wanted again, and the request scheduler
/// decides the wait.
pub fn on_load_rejected(key: u64, how: Ended) {
    let Some((kind, load)) = on_ended(key, how) else {
        return;
    };
    *loader().at(kind, load) = None;
}
