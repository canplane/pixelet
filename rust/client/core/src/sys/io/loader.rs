// pxl-core/sys/io/loader.rs

//! The assets' files, each read at most once: the catalog, the first asked
//! (`catalog`), the models and the images its slots name, and the Earth's
//! sphere. An asset is a want in the requests as a tile is, on the same
//! credits, wanted until it arrives; a slot names no file until the catalog
//! lands, which its reader takes as one still loading.
//!
//! An open that fails leaves its asset unasked, to be asked again; one the
//! heap refused room waits as a tile's body does (sys/io/request.rs
//! `Wait::Room`). Bytes that do not read are final, as asking again brings
//! the same. A model is its file's bytes, handed once to the scene, which
//! parses them (`take_gltf`). An image is its file's bytes, handed to the
//! renderer under the asset's number until its texture lands (pxl-render
//! `Gpu::image`), and read again where the device is lost.

use pxl_spec::asset;
use pxl_spec::asset::catalog::{Catalog, Slot};
use pxl_spec::bake::publication::BaseLayer;
use xpute_core::global::Global;
use xpute_core::ipc::fs::Fd;
use xpute_core::sched::edge::{runtime_turn, yield_if_spent};
use xpute_core::sched::task;

use crate::sys::io::demand::Order;
use crate::sys::io::failure::{Ended, Failure};
use crate::sys::io::file::{self, Opened, Owner, Reading};
use crate::sys::io::file_spec::{root, CATALOG};
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::io::source;
use crate::sys::load::{Load, LoadKind};
use crate::sys::sched::edge::{request_world_tick, world_edge};

enum Entry {
    Pending,
    /// Its bytes in its arrival's task, its credit given back.
    Arrived,
    /// Its file's bytes, until its reader takes them in: a model's until the
    /// scene takes them, an image's until its texture lands.
    Bytes(Vec<u8>),
    /// Taken in where its reader keeps it, its bytes gone.
    Landed,
    Failed,
}

const KINDS: usize = LoadKind::ALL.len();

struct Loader {
    /// By kind, then by number below the asset root, grown to the highest
    /// asked, none unasked: one file can be wanted as several kinds.
    entries: [Vec<Option<Entry>>; KINDS],
    sphere: [Option<Entry>; KINDS],
    catalog: Option<Catalog>,
    /// `load_wants`' own, so saying them every turn allocates nothing.
    wants: Vec<(u64, Order)>,
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        entries: Default::default(),
        sphere: Default::default(),
        catalog: None,
        wants: Vec::new(),
    })
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

/// The catalog the client plugs in (pxl-spec asset/catalog.rs), asked for
/// when first read; none while it loads.
pub fn catalog() -> Option<&'static Catalog> {
    let l = loader();
    l.at(LoadKind::Catalog, Load::File(CATALOG))
        .get_or_insert(Entry::Pending);
    l.catalog.as_ref()
}

/// The file `slot` names; none while the catalog loads.
pub fn load_of(slot: Slot) -> Option<Load> {
    catalog().map(|c| Load::File(c.file(slot)))
}

/// A model's file's bytes, handed over once: none while it loads, and once
/// taken.
pub fn take_gltf(load: Load) -> Option<Vec<u8>> {
    let entry = loader()
        .at(LoadKind::Gltf, load)
        .get_or_insert(Entry::Pending);
    let Entry::Bytes(bytes) = entry else {
        return None;
    };
    let bytes = core::mem::take(bytes);
    *entry = Entry::Landed;
    Some(bytes)
}

/// An image as the renderer takes it: the asset's number and its file's
/// bytes, no bytes once its texture landed; none while it loads.
pub fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    match loader()
        .at(LoadKind::Image, load)
        .get_or_insert(Entry::Pending)
    {
        Entry::Bytes(bytes) => Some((load.id(), bytes.as_slice())),
        Entry::Landed => Some((load.id(), &[])),
        _ => None,
    }
}

/// Kind in the high word, as one file can be wanted as more than one.
pub fn id_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | u64::from(load.id())
}

pub fn of_id(id: u64) -> Option<(LoadKind, Load)> {
    let kind = LoadKind::ALL.into_iter().find(|&k| k as u64 == id >> 32)?;
    Some((kind, Load::of(id as u32)?))
}

/// What is asked and not yet read, by id, as a stream's wants are said: by
/// kind, the files by number and the sphere past them, once its source
/// declares the version it is opened under.
pub fn load_wants() -> &'static [(u64, Order)] {
    let l = loader();
    l.wants.clear();
    for kind in LoadKind::ALL {
        let files = (0..l.entries[kind as usize].len() as u32).map(Load::File);
        for load in files.chain(source::top().map(|_| Load::Sphere)) {
            if matches!(l.at(kind, load), Some(Entry::Pending)) {
                l.wants.push((id_of(kind, load), Order::LAST));
            }
        }
    }
    &l.wants
}

/// A model and an image of one load are one file; which it is read as is
/// its attempt's id's.
pub fn open_asset(key: u64, load: Load) {
    let root = match load {
        Load::Sphere => BaseLayer::earth as u32,
        Load::File(_) => root::ASSET,
    };
    file::open(root, load.path(), Owner::Load { key });
}

/// The open answered: the bytes' room taken now, the descriptor closed and
/// the credit given back, so taking the bytes in holds none of them.
pub fn on_opened(fd: Fd, key: u64, opened: Opened) {
    let what = attempt_of(key).and_then(|(_, id)| of_id(id));
    let Opened::Bytes(n) = opened else {
        settle(key, Ended::Failed(Failure::from(opened)), None);
        if let Some((kind, load)) = what {
            *loader().at(kind, load) = None;
        }
        return;
    };
    let Some((kind, load)) = what else {
        return file::close(fd);
    };
    match file::read_whole(fd, n) {
        Ok(reading) => {
            settle(key, Ended::Ok, None);
            *loader().at(kind, load) = Some(Entry::Arrived);
            // The pool holds a task a file in flight (sys/load.rs `LOAD_FILES`).
            let spawned = task::spawn(arrival(kind, load, reading), 0);
            xpute_kit::ensure!(spawned, ENOSPC, load.id());
        }
        Err(refused) => settle(key, Ended::Refused(refused), None),
    }
}

/// A file read whole, taken in by its own task once the runtime's head has
/// passed its read: where the turn is spent before the tasks run, each
/// advances a step a turn, so no arrival waits on another's.
async fn arrival(kind: LoadKind, load: Load, mut reading: Reading) {
    let bytes = loop {
        match file::take_read(reading) {
            Ok(bytes) => break bytes,
            Err(unread) => reading = unread,
        }
        runtime_turn(world_edge).await;
    };
    yield_if_spent(world_edge).await;
    let l = loader();
    let entry = match kind {
        LoadKind::Gltf => Entry::Bytes(bytes),
        // A header sizes it (pxl-spec `asset::image_size`).
        LoadKind::Image if asset::image_size(&bytes).is_some() => Entry::Bytes(bytes),
        LoadKind::Image => Entry::Failed,
        LoadKind::Catalog => {
            l.catalog = Catalog::read(&bytes);
            match l.catalog {
                Some(_) => Entry::Landed,
                None => Entry::Failed,
            }
        }
    };
    *l.at(kind, load) = Some(entry);
    // A turn hands it on.
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
    let image = LoadKind::Image as usize;
    for entry in l.entries[image].iter_mut().chain([&mut l.sphere[image]]) {
        if matches!(entry, Some(Entry::Landed)) {
            *entry = None;
        }
    }
}
