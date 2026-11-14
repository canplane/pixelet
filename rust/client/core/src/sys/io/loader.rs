// pxl-core/sys/io/loader.rs

//! The asset files, each read at most once: the catalog, which is asked for
//! first (`catalog`), the models and images its slots name, and the Earth's
//! sphere. An asset is a want in the request scheduler just as a tile is, on
//! the same credits, and it stays wanted until it arrives. A slot names no
//! file until the catalog lands, and its reader treats it as still loading.
//!
//! A read the host canceled leaves its asset unasked, so that it is asked
//! again (`arrival`); any other failed read is final. Bytes that do not read
//! are final, because asking again brings the same bytes. A model's bytes are
//! handed once to the scene, which parses them (`take_gltf`). An image's
//! bytes are handed to the renderer under the asset's number until its
//! texture lands (pxl-render `Gpu::image`), and they are read again if the
//! device is lost.

use pxl_spec::asset;
use pxl_spec::asset::catalog::{Catalog, Slot};
use pxl_global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::demand::Order;
use crate::sys::io::failure::{exit_of, Exit, Failure};
use crate::sys::io::file;
use crate::spec::file::CATALOG;
use crate::spec::manifest::mem;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::io::source;
use crate::sys::load::{id_of, of_id, Load, LoadKind, LOAD_FILE_CAP};
use crate::sys::sched::executor;
use crate::sys::sched::{self, invalidate};

enum Entry {
    Pending,
    /// The file is being read, or its bytes are with its arrival task, and
    /// the credit is given back.
    InTask,
    /// The file's bytes, kept until the reader takes them in: a model's until
    /// the scene takes them, an image's until its texture lands.
    Bytes(Box<[u8]>),
    /// The reader has taken the file in and keeps it, and the bytes are gone.
    InReader,
    Failed,
}

const KIND_CNT: usize = LoadKind::ALL.len();

struct Loader {
    /// The entries by kind, then by number after ASSET. A row grows
    /// to the highest number asked, and an unasked number holds None. One
    /// file can be wanted as several kinds, so each kind has its own row.
    entries: [Vec<Option<Entry>>; KIND_CNT],
    sphere: [Option<Entry>; KIND_CNT],
    catalog: Option<Catalog>,
    /// Scratch for `load_wants`, so that declaring the wants every frame
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
pub(crate) fn catalog() -> Option<&'static Catalog> {
    let l = loader();
    l.at(LoadKind::Catalog, Load::File(CATALOG))
        .get_or_insert(Entry::Pending);
    l.catalog.as_ref()
}

/// The file `slot` names; none while the catalog loads.
pub(crate) fn load_of(slot: Slot) -> Option<Load> {
    catalog().map(|c| Load::File(c.file(slot)))
}

/// A model's bytes, handed over once. It is None while the file loads and
/// after the bytes are taken.
pub(crate) fn take_gltf(load: Load) -> Option<Box<[u8]>> {
    let entry = loader()
        .at(LoadKind::Gltf, load)
        .get_or_insert(Entry::Pending);
    if !matches!(entry, Entry::Bytes(_)) {
        return None;
    }
    let Entry::Bytes(bytes) = core::mem::replace(entry, Entry::InReader) else {
        return None;
    };
    Some(bytes)
}

/// An image as the renderer takes it: the asset's number and the file's
/// bytes. The bytes are empty once the texture has landed, and the whole is
/// None while the file loads.
pub(crate) fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    match loader()
        .at(LoadKind::Image, load)
        .get_or_insert(Entry::Pending)
    {
        Entry::Bytes(bytes) => Some((load.id(), &bytes[..])),
        Entry::InReader => Some((load.id(), &[])),
        _ => None,
    }
}

/// The files asked for and not yet read, by id, in the form a stream
/// declares its wants. They come by kind, the files by number and the sphere
/// after them. The sphere is wanted only once its source declares the
/// version it is opened under.
pub(crate) fn load_wants() -> &'static [(u64, Order)] {
    let l = loader();
    l.wants.clear();
    for kind in LoadKind::ALL {
        let files = (0..l.entries[kind as usize].len() as u32).map(Load::File);
        for load in files.chain(source::min_zoom().map(|_| Load::Sphere)) {
            if matches!(l.at(kind, load), Some(Entry::Pending)) {
                l.wants.push((id_of(kind, load), Order::LAST));
            }
        }
    }
    &l.wants
}

/// Reads the file of `load` for the attempt `key`, in a task of its own. A
/// model and an image of one load are the same file, and the attempt's id
/// says which it is read as.
pub(crate) fn open_asset(key: u64, load: Load) {
    let Some((_, id)) = attempt_of(key) else { return };
    let Some((kind, _)) = of_id(id) else { return };
    // The task's pool has room for one for each file in flight
    // (sys/load.rs `LOAD_FILE_CAP`), so the spawn cannot fail.
    executor::spawner().spawn(arrival(key, kind, load).or_bug(Errno::ENOSPC));
}

/// Reads a file whole and takes it in, in a task of its own, and gives the
/// attempt's credit back once it is read. Where the entry is spent before the
/// tasks run, each arrival still advances one step an entry, so no arrival
/// waits on another. A file that cannot be read, absent among them, fails
/// its load; a canceled read ends aborted (`exit_of`), and the load is asked
/// again.
#[embassy_executor::task(pool_size = LOAD_FILE_CAP)]
async fn arrival(key: u64, kind: LoadKind, load: Load) {
    let read = file::read_whole(&load.path(), mem::MIN_HEAP_SZ, Some(key), file::heap)
        .await
        .map(Vec::into_boxed_slice);
    let l = loader();
    let bytes = match read {
        Ok(bytes) => {
            settle(key, Exit::Ok, None);
            *l.at(kind, load) = Some(Entry::InTask);
            bytes
        }
        Err(errnum) => {
            let exit = exit_of(Failure::from(errnum));
            settle(key, exit, None);
            *l.at(kind, load) = if exit == Exit::Aborted { None } else { Some(Entry::Failed) };
            invalidate();
            return;
        }
    };
    sched::yield_if_spent().await;
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
    // A frame hands the entry on to its reader.
    invalidate();
}

/// The renderer has made the texture of the image numbered `image`
/// (pxl-render `take_complete`), so the file's bytes are let go.
pub(crate) fn on_image_complete(image: u32) {
    let Some(load) = Load::of(image) else { return };
    let entry = loader().at(LoadKind::Image, load);
    if matches!(entry, Some(Entry::Bytes(_))) {
        *entry = Some(Entry::InReader);
    }
}

/// The device was lost, and with it every texture an image landed in. Each
/// such image is read again when it is next asked for. An image whose bytes
/// are still held is handed to the renderer again.
pub(crate) fn on_images_lost() {
    let l = loader();
    let image = LoadKind::Image as usize;
    for entry in l.entries[image].iter_mut().chain([&mut l.sphere[image]]) {
        if matches!(entry, Some(Entry::InReader)) {
            *entry = None;
        }
    }
}
