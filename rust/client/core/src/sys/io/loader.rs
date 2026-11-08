// pxl-core/sys/io/loader.rs

//! The asset files, each read at most once: the catalog, which is asked for
//! first (`catalog`), the models and images its slots name, and the Earth's
//! sphere. An asset is a want in the request scheduler just as a tile is, on
//! the same credits, and it stays wanted until it arrives. A slot names no
//! file until the catalog lands, and its reader treats it as still loading.
//!
//! An open that fails leaves its asset unasked, so that it is asked again.
//! An open the heap refused room waits as a tile's body does
//! (sys/io/request.rs `Wait::Room`). Bytes that do not read are final,
//! because asking again brings the same bytes. A model's bytes are handed
//! once to the scene, which parses them (`take_gltf`). An image's bytes are
//! handed to the renderer under the asset's number until its texture lands
//! (pxl-render `Gpu::image`), and they are read again if the device is lost.

use pxl_spec::asset;
use pxl_spec::asset::catalog::{Catalog, Slot};
use pxl_spec::bake::publication::BaseLayer;
use xpute_core::global::Global;
use xpute_core::ipc::fs::Fd;
use xpute_core::sched::edge::{runtime_turn, yield_if_spent};
use xpute_core::sched::task;

use crate::sys::io::demand::Order;
use crate::sys::io::failure::{Exit, Failure};
use crate::sys::io::file::{self, Opened, Owner, Reading};
use crate::sys::io::file_spec::{root, CATALOG};
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::io::source;
use crate::sys::load::{Load, LoadKind};
use crate::sys::sched::edge::{request_world_tick, world_edge};

enum Entry {
    Pending,
    /// The bytes are with the file's arrival task, and the credit is given
    /// back.
    InTask,
    /// The file's bytes, kept until the reader takes them in: a model's until
    /// the scene takes them, an image's until its texture lands.
    Bytes(Vec<u8>),
    /// The reader has taken the file in and keeps it, and the bytes are gone.
    InReader,
    Failed,
}

const KINDS: usize = LoadKind::ALL.len();

struct Loader {
    /// The entries by kind, then by number below the asset root. A row grows
    /// to the highest number asked, and an unasked number holds None. One
    /// file can be wanted as several kinds, so each kind has its own row.
    entries: [Vec<Option<Entry>>; KINDS],
    sphere: [Option<Entry>; KINDS],
    catalog: Option<Catalog>,
    /// Scratch for `load_wants`, so that declaring the wants every turn
    /// allocates nothing.
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

/// The catalog the client plugs in (pxl-spec asset/catalog.rs). The first
/// read asks for it, and it is None while it loads.
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

/// A model's bytes, handed over once. It is None while the file loads and
/// after the bytes are taken.
pub fn take_gltf(load: Load) -> Option<Vec<u8>> {
    let entry = loader()
        .at(LoadKind::Gltf, load)
        .get_or_insert(Entry::Pending);
    let Entry::Bytes(bytes) = entry else {
        return None;
    };
    let bytes = core::mem::take(bytes);
    *entry = Entry::InReader;
    Some(bytes)
}

/// An image as the renderer takes it: the asset's number and the file's
/// bytes. The bytes are empty once the texture has landed, and the whole is
/// None while the file loads.
pub fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    match loader()
        .at(LoadKind::Image, load)
        .get_or_insert(Entry::Pending)
    {
        Entry::Bytes(bytes) => Some((load.id(), bytes.as_slice())),
        Entry::InReader => Some((load.id(), &[])),
        _ => None,
    }
}

/// A want's id, with the kind in the high word, because one file can be
/// wanted as more than one kind.
pub fn id_of(kind: LoadKind, load: Load) -> u64 {
    (kind as u64) << 32 | u64::from(load.id())
}

pub fn of_id(id: u64) -> Option<(LoadKind, Load)> {
    let kind = LoadKind::ALL.into_iter().find(|&k| k as u64 == id >> 32)?;
    Some((kind, Load::of(id as u32)?))
}

/// The files asked for and not yet read, by id, in the form a stream
/// declares its wants. They come by kind, the files by number and the sphere
/// after them. The sphere is wanted only once its source declares the
/// version it is opened under.
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

/// Opens the file of `load`. A model and an image of one load are the same
/// file, and the attempt's id says which it is read as.
pub fn open_asset(key: u64, load: Load) {
    let root = match load {
        Load::Sphere => BaseLayer::earth as u32,
        Load::File(_) => root::ASSET,
    };
    file::open(root, load.path(), Owner::Load { key });
}

/// Takes the answer to an asset's open. The room for the bytes is taken now,
/// the descriptor is closed and the credit is given back, so taking the
/// bytes in holds neither.
pub fn on_opened(fd: Fd, key: u64, opened: Opened) {
    let what = attempt_of(key).and_then(|(_, id)| of_id(id));
    let Opened::Bytes(n) = opened else {
        settle(key, Exit::Failed(Failure::from(opened)), None);
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
            settle(key, Exit::Ok, None);
            *loader().at(kind, load) = Some(Entry::InTask);
            // The task pool has room for one task for each file in flight
            // (sys/load.rs `LOAD_FILES`), so the spawn cannot fail.
            let spawned = task::spawn(arrival(kind, load, reading), 0);
            xpute_kit::ensure!(spawned, ENOSPC, load.id());
        }
        Err(refused) => settle(key, Exit::Refused(refused), None),
    }
}

/// Takes in a file read whole, in a task of its own, once the runtime's head
/// has passed its read. Where the turn is spent before the tasks run, each
/// arrival still advances one step a turn, so no arrival waits on another.
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
        // An image is kept only where its header gives its size (pxl-spec
        // `asset::image_size`).
        LoadKind::Image if asset::image_size(&bytes).is_some() => Entry::Bytes(bytes),
        LoadKind::Image => Entry::Failed,
        LoadKind::Catalog => {
            l.catalog = Catalog::read(&bytes);
            match l.catalog {
                Some(_) => Entry::InReader,
                None => Entry::Failed,
            }
        }
    };
    *l.at(kind, load) = Some(entry);
    // A turn hands the entry on to its reader.
    request_world_tick();
}

/// The renderer has made the texture of the image numbered `image`
/// (pxl-render `take_complete`), so the file's bytes are let go.
pub fn on_image_complete(image: u32) {
    let Some(load) = Load::of(image) else { return };
    let entry = loader().at(LoadKind::Image, load);
    if matches!(entry, Some(Entry::Bytes(_))) {
        *entry = Some(Entry::InReader);
    }
}

/// The device was lost, and with it every texture an image landed in. Each
/// such image is read again when it is next asked for. An image whose bytes
/// are still held is handed to the renderer again.
pub fn on_images_lost() {
    let l = loader();
    let image = LoadKind::Image as usize;
    for entry in l.entries[image].iter_mut().chain([&mut l.sphere[image]]) {
        if matches!(entry, Some(Entry::InReader)) {
            *entry = None;
        }
    }
}
