// pxl-kernel/io/load.rs

//! Shared loading of models and images, so each asset is fetched and parsed
//! at most once. An asset is a want in the request scheduler like a tile, sharing
//! its credits, and stays wanted until it arrives.
//!
//! A read that did not arrive is retried, and one the heap refused room for
//! is asked again once the heap gives some back; bytes that do not parse are
//! final, since asking again returns the same bytes. An image is its file's
//! bytes, kept for as long as the kernel runs and handed to the renderer
//! under the asset's number, which makes its texture on whatever device
//! there is (pxl-render `Gpu::image`): io is transport, and the device the
//! renderer's.

use std::rc::Rc;

use pxl_render::asset::gltf::{parse_gltf, Gltf};
use xpute_guest::sched::edge::{next_turn, yield_if_spent};

use crate::abi::host::root;
use crate::asset::{Load, LoadKind, LOADS};
use crate::io::demand::Ask;
use crate::io::failure::Ended;
use crate::io::file::{self, Opened, Owner, Path};
use crate::io::host::Refused;
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
enum Entry {
    Pending,
    /// Its file in its arrival's task, being read.
    Arrived,
    /// The heap refused its bytes room: a wait, not a failure, so it is
    /// pending again once the heap gives some back (`load_wants`).
    Refused(Refused),
    /// A parsed model.
    Model(Rc<Gltf>),
    /// An image's bytes, sized by its header.
    Image(Vec<u8>),
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
/// bytes, once read; none while it loads.
pub fn load_image(load: Load) -> Option<(u32, &'static [u8])> {
    let entry = loader().at(LoadKind::Image, load.number() as usize);
    match entry {
        Some(Entry::Image(file)) => Some((load.number(), file.as_slice())),
        Some(_) => None,
        None => {
            *entry = Some(Entry::Pending);
            None
        }
    }
}

pub fn load_failed(load: Load) -> bool {
    let n = load.number() as usize;
    loader().entries.iter().any(|kind| matches!(kind[n], Some(Entry::Failed)))
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
            if pending(l.at(kind, n)) {
                l.wants.push((subject_of(kind, load), Ask::PLAIN));
            }
        }
    }
    &l.wants
}

/// Whether a load is wanted: pending, or refused by a heap that has since
/// given room back.
fn pending(entry: &mut Option<Entry>) -> bool {
    if let Some(Entry::Refused(refused)) = entry {
        if refused.over() {
            *entry = Some(Entry::Pending);
        }
    }
    matches!(entry, Some(Entry::Pending))
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

/// A model and an image of one load are the same file; which it is read as
/// is its attempt's subject's (`opened`).
pub fn open_asset(credit: Credit, load: Load) {
    let owner = Owner::Load { key: credit.key() };
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
    let Some(fd) = file::held_for(|o| *o == Owner::Load { key }) else {
        return false;
    };
    file::close(fd);
    load_rejected(key, Ended::Aborted);
    true
}

pub fn opened(fd: u32, key: u64, opened: Opened) {
    let Opened::Bytes(n) = opened else {
        load_rejected(key, Ended::Failed(opened.failure()));
        return;
    };
    let Some((kind, load)) = ended(key, Ended::Ok) else {
        file::close(fd);
        return;
    };
    arrive(kind, load.number() as usize, fd, n);
}

/// An image's bytes, read: kept, where a header gives its size
/// (pxl-spec `asset::image_size`); a file no header sizes is the asset's
/// own failure, so it is final.
fn image_arrived(n: usize, file: Vec<u8>) {
    *loader().at(LoadKind::Image, n) = Some(if pxl_spec::asset::image_size(&file).is_some() { Entry::Image(file) } else { Entry::Failed });
    settled();
    // A turn hands it on.
    request_world_tick();
}

/// A file opened, read in its own task: no arrival waits on another's
/// parse or landing. Where the turn's quota is spent before the tasks run, as
/// it is while the frame outweighs its share, each task advances a step a
/// turn, so k arrivals advance k steps, not one.
fn arrive(kind: LoadKind, n: usize, fd: u32, bytes: u32) {
    *loader().at(kind, n) = Some(Entry::Arrived);
    // The pool holds a task for each kind of every load (TASKS), and each
    // has one file in flight at most, so there is always room.
    let spawned = xpute_guest::sched::task::spawn(arrival(kind, n, fd, bytes), crate::sched::edge::TaskPriority::Arrival as u8);
    xpute_kit::ensure!(spawned, ENOSPC, n);
}

/// An asset file arrived, read whole: a model parsed, an image put into its
/// texture.
async fn arrival(kind: LoadKind, n: usize, fd: u32, bytes: u32) {
    yield_if_spent(world_edge).await;
    // SAFETY: `buf` is held across the turn the host fills it in.
    let buf = match unsafe { file::read_whole(fd, bytes) } {
        Ok(buf) => buf,
        Err(refused) => {
            *loader().at(kind, n) = Some(Entry::Refused(refused));
            return;
        }
    };
    // In once the host has run the turn's records.
    next_turn(world_edge).await;
    if kind == LoadKind::Image {
        image_arrived(n, buf);
        return;
    }
    let entry = match parse_gltf(&buf, &[], || yield_if_spent(world_edge)).await {
        Ok(g) => Entry::Model(Rc::new(g)),
        Err(_) => Entry::Failed,
    };
    *loader().at(LoadKind::Gltf, n) = Some(entry);
    settled();
    request_world_tick();
}

/// The transport failed, not the asset: it is wanted again, and the request scheduler
/// decides the wait.
pub fn load_rejected(key: u64, how: Ended) {
    let Some((kind, load)) = ended(key, how) else { return };
    *loader().at(kind, load.number() as usize) = None;
    settled();
}
