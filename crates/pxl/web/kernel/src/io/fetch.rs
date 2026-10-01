// pxl-kernel/io/fetch.rs

//! The world's reads at the shell's edge. What the kernel asks for is an
//! **object it names** — which bake it belongs to and its name inside that
//! bake (`DemandName`) — opened as a file below the bake's root (io/file.rs),
//! and the host is the one that knows what address that is and what protocol
//! reaches it. Nothing here spells a URL, a scheme or a status.
//!
//! What comes back is one of four: the bytes, no such object (ENOENT), the
//! read was stopped (the kernel closed it), the read failed. The host decides
//! which — a protocol's own way of saying "there is none" is the host's to
//! read — and the open's answer takes a block, reads the file into it and
//! closes it; `fetch_task`, on a turn's quota, decodes it once it is in. What
//! an absent object *means* for its category is decided here, and that is
//! world knowledge rather than transport.
//!
//! Every body is a heap block the kernel owns throughout. A body is decoded or
//! read and released in the same command. A terrain body does not outlive its
//! command — the page it carries is written into `.fixed.terrain`, whose
//! record is the page's own coordinate. POI summary packets are decoded into
//! bounded records and their bodies are released in the same command.

use core::fmt;

use xpute_core::status::errno::Errno;
use xpute_runtime::ipc::sys::Fd;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free, Block};
use xpute_runtime::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::io::file::{self, Owner};
use crate::io::sched::{attempt_of, settle};
use crate::layer::geocode::{repository as geocode_repository, store as geocode_store};
use crate::layer::terrain::page::{adopt_ocean, OCEAN_KEY};
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::change::world_edge;
use crate::world::pyramid::Layer;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::place_page::group_name;
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::bake::quadkey::page_name;
use pxl_spec::space::pyramid::LevelPage;

/// A started attempt's object, named inside its bake — the path it is opened
/// at. Which bake is `Category::bake`, the root it is opened below.
pub struct DemandName(pub Category, pub u64);

impl fmt::Display for DemandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self;
        match key.0 {
            Category::terrain => write!(f, "{}", page_name(LevelPage::of_subject(key.1))),
            // The one tile the stream has, at the name the bake gives it.
            Category::ocean => f.write_str(OCEAN_KEY),
            Category::poi => write!(f, "{}", page_name(LevelPage::of_subject(key.1))),
            Category::geocode => write!(f, "{}", group_name(key.1)),
            // The one packet the stream has, at the name the bake gives it.
            Category::geocode_dict => write!(f, "{}", geocode_repository::dict_name()),
            // An asset is named by its content and belongs to no bake, so it
            // is opened below the assets' root (io/load.rs) and never here.
            Category::asset => Ok(()),
        }
    }
}

/// Opens the object the scheduler's attempt `key` is for.
pub fn start(key: u64, category: Category, subject: u64) {
    let Some(bake) = category.bake() else { return };
    let name = DemandName(category, subject).to_string();
    file::open(bake as u32, &name, Owner::Fetch { key, at_ms: now() });
}

/// Stops the read of attempt `key`, if one is open: closed, and settled as
/// stopped at once, since nothing is said of a closed descriptor again.
pub fn cancel(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| matches!(o, Owner::Fetch { key: k, .. } if *k == key)) else {
        return false;
    };
    file::close(fd);
    settle(key, Outcome::aborted, None, Failure::default());
    true
}

fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, acquired by `opened` and released once.
        unsafe { free(block) };
    }
}

/// A body's bytes, read where the host copied them.
fn view<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block is one `opened` acquired for this body alone,
    // `bytes` long, and it is not released while the view is read.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at), bytes as usize) }
}

/// What an open came to.
enum Got {
    /// The file, read into a block of its own; None for an empty one.
    Read(Option<Block>),
    /// There is no such object.
    Absent,
    Failed(Failure),
}

struct Arrival {
    key: u64,
    got: Got,
    net_ms: f64,
}

static ARRIVED: Global<Vec<Arrival>> = Global::new();
static ARRIVALS: Global<Notify> = Global::new();

fn arrived() -> &'static mut Vec<Arrival> {
    ARRIVED.get(Vec::new)
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// The host's answer to a read's open: a block taken for the file and the
/// file read into it, or why there is none. `detail` is whatever number the
/// host refused with, kept for the profiler to print; nothing branches on it,
/// which is what keeps a protocol out of here.
pub fn opened(fd: Fd, key: u64, at_ms: f64, res: i32, detail: u32) {
    let got = if res < 0 {
        file::close(fd);
        failure(res, detail)
    } else if res == 0 {
        file::close(fd);
        Got::Read(None)
    } else if let Some(block) = alloc(res as u32) {
        // SAFETY: a block just acquired, `res` long, and no one else's.
        file::read(fd, unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(block.0), res as usize) });
        file::close(fd);
        Got::Read(Some(block))
    } else {
        file::close(fd);
        Got::Failed(Errno::ENOMEM.into())
    };
    arrived().push(Arrival { key, got, net_ms: now() - at_ms });
    arrivals().notify();
}

/// What an open that did not give a file says: an object that is not there,
/// or the errno it failed with, as the profiler counts it.
fn failure(res: i32, detail: u32) -> Got {
    if res == Errno::ENOENT as i32 {
        return Got::Absent;
    }
    Got::Failed(Failure {
        errno: res,
        detail: detail as u16,
        at: None,
    })
}

/// The fetches' task: the arrivals taken together, then each decoded and
/// settled on a later turn, when their bytes are in, as many as the quota
/// lets and one at least. A body is one unit.
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            let received: Vec<_> = std::mem::take(arrived());
            next_turn(world_edge).await;
            for arrival in received {
                yield_if_spent(world_edge).await;
                fetch_arrived(arrival);
            }
        }
    }
}

fn fetch_arrived(Arrival { key, got, net_ms }: Arrival) {
    let block = match got {
        Got::Read(block) => block,
        _ => None,
    };
    // A key the scheduler does not hold is one a reset forgot.
    let Some((category, subject)) = attempt_of(key) else {
        release(block);
        return;
    };
    let mut phases = Phases::default();
    phases.push(TaskPhaseName::net_ms, net_ms);
    if let Some((_, bytes)) = block {
        phases.push(TaskPhaseName::bytes, bytes as f64);
    }
    let (outcome, fail) = match got {
        Got::Read(_) => match adopt(category, subject, block, &mut phases) {
            Ok(()) => (Outcome::ok, Failure::default()),
            Err(fail) => (Outcome::error, fail),
        },
        // There is no such object, and for most categories that is an answer
        // rather than a fault — which of the two is the category's own and
        // is the one piece of this the world decides.
        Got::Absent => match absent(category, subject) {
            Ok(()) => (Outcome::ok, Failure::default()),
            Err(fail) => (Outcome::error, fail.into()),
        },
        Got::Failed(fail) => (Outcome::error, fail),
    };
    settle(key, outcome, Some(phases), fail);
}

/// There is no such object. What that means is the category's: open water
/// for a terrain page, no label for a geocode group. POI snapshots must exist.
/// The shared ocean tile and the dictionary are the two that must exist, so
/// their absence is a fault.
fn absent(category: Category, subject: u64) -> Result<(), Errno> {
    match category {
        Category::terrain => Terrain::absent(LevelPage::of_subject(subject)),
        Category::poi => PoiPage::absent(LevelPage::of_subject(subject)),
        Category::geocode => geocode_store::set_group(subject, None),
        // A bake with no dictionary is one whose groups name nothing: an
        // answer, so the demand is not made again.
        Category::geocode_dict => geocode_store::set_dict(None),
        Category::ocean => return Err(Errno::ENOENT),
        // An asset's read is settled where it is asked for (io/load.rs) and
        // never arrives here.
        Category::asset => return Err(Errno::ENOTRECOVERABLE),
    };
    Ok(())
}

/// The bytes arrived. The block is released here, unless the page keeps it.
fn adopt(category: Category, subject: u64, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    // A read that answered with nothing is not an object; only `absent` says
    // there is none, and it carries no body.
    let Some(b) = block else { return Err(Errno::EBADMSG.into()) };
    match category {
        Category::terrain => {
            let result = Terrain::pyramid().adopt(LevelPage::of_subject(subject), view(b), phases);
            release(block);
            result
        }
        Category::ocean => {
            let result = adopt_ocean(view(b));
            release(block);
            result
        }
        // SAFETY of each `open_*` below: the block is the heap's, given for
        // this body alone; the page or packet keeps it and the store releases
        // it with them.
        Category::poi => {
            let result = PoiPage::pyramid().adopt(LevelPage::of_subject(subject), view(b), phases);
            release(block);
            result
        }
        Category::geocode => match unsafe { geocode_repository::open_group(b.0, b.1) } {
            Ok(resident) => {
                geocode_store::set_group(subject, Some(resident));
                Ok(())
            }
            Err(err) => {
                release(block);
                Err(err.into())
            }
        },
        // As `absent`: an asset never arrives here.
        Category::asset => Err(Errno::ENOTRECOVERABLE.into()),
        Category::geocode_dict => match unsafe { geocode_repository::open_dict(b.0, b.1) } {
            Ok(resident) => {
                geocode_store::set_dict(Some(resident));
                Ok(())
            }
            Err(err) => {
                release(block);
                Err(err.into())
            }
        },
    }
}
