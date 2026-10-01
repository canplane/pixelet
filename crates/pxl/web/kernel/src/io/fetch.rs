// pxl-kernel/io/fetch.rs

//! The world's reads at the shell's edge. What the kernel asks for is an
//! **object it names** — which bake it belongs to and its name inside that
//! bake (`DemandName`) — and the shell is the one that knows what address
//! that is and what protocol reaches it. Nothing here spells a URL, a scheme
//! or a status.
//!
//! What comes back is one of four: the bytes, no such object, the read was
//! stopped, the read failed. The host decides which — a protocol's own way of
//! saying "there is none" is the host's to read — and `fetch_task`, on a
//! turn's quota, takes a block and has the host copy the body into it, the
//! way a driver reads what a device holds, and decodes it. What an absent
//! object *means* for its category is decided here, and that is world
//! knowledge rather than transport.
//!
//! Every body is a heap block the kernel owns throughout. A body is decoded or
//! read and released in the same command. A terrain body does not outlive its
//! command — the page it carries is written into `.fixed.terrain`, whose
//! record is the page's own coordinate. POI summary packets are decoded into
//! bounded records and their bodies are released in the same command.

use core::fmt;

use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free, Block};
use xpute_runtime::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::global::Global;
use crate::host::body_read;
use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::io::policy;
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
use xpute_runtime::abi::cmd::{FetchDone, FetchOutcome};

/// A started attempt's object, named inside its bake — what is written where
/// the read is asked for. Which bake is `Category::bake`, and the two
/// together are all the host is given.
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
            // is asked for by its own command and never written here.
            Category::asset => Ok(()),
        }
    }
}

/// A block of its own for the body the host holds as `body`, `bytes` long,
/// which the host copies it into after this turn: None when there is none,
/// NoRoom when the heap has no block for it. The host lets the body go either
/// way.
fn receive((body, bytes): (u32, u32)) -> Result<Option<Block>, FetchFail> {
    if bytes == 0 {
        return Ok(None);
    }
    let Some(block) = alloc(bytes) else {
        body_read(body, &mut []);
        return Err(FetchFail::NoRoom);
    };
    // SAFETY: a block just acquired, `bytes` long, and no one else's.
    body_read(body, unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(block.0), bytes as usize) });
    Ok(Some(block))
}

fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, acquired by `receive` and released once.
        unsafe { free(block) };
    }
}

/// A body's bytes, read where the host copied them.
fn view<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block is one `receive` acquired for this body alone,
    // `bytes` long, and it is not released while the view is read.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at), bytes as usize) }
}

static ARRIVED: Global<Vec<FetchDone>> = Global::new();
static ARRIVALS: Global<Notify> = Global::new();

fn arrived() -> &'static mut Vec<FetchDone> {
    ARRIVED.get(Vec::new)
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// The host's report that a read finished, queued for `fetch_task`. `detail`
/// is whatever number the host refused with, kept for the profiler to print;
/// nothing branches on it, which is what keeps a protocol out of here.
pub fn fetch_queued(update: FetchDone) {
    arrived().push(update);
    arrivals().notify();
}

/// The fetches' task: the arrivals' bodies asked for together, then each
/// decoded and settled on a later turn, when they are in, as many as the
/// quota lets and one at least. A body is one unit.
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            let received: Vec<_> = arrived().drain(..).map(|u| (receive((u.body, u.body_bytes)), u)).collect();
            next_turn(world_edge).await;
            for (block, update) in received {
                yield_if_spent(world_edge).await;
                fetch_arrived(update, block);
            }
        }
    }
}

fn fetch_arrived(
    FetchDone {
        key, outcome, fail, detail, net_ms, ..
    }: FetchDone,
    block: Result<Option<Block>, FetchFail>,
) {
    let (outcome, fail, block) = match block {
        Ok(block) => (
            outcome,
            Failure {
                fail,
                detail: detail as u16,
                at: None,
            },
            block,
        ),
        Err(no_room) => (FetchOutcome::Error, no_room.into(), None),
    };
    // A hash the scheduler does not hold is one a reset forgot; its body is
    // already back.
    let Some((category, subject)) = attempt_of(key) else {
        release(block);
        return;
    };
    let mut phases = Phases::default();
    phases.push(TaskPhaseName::net_ms, net_ms);
    if let Some((_, bytes)) = block {
        phases.push(TaskPhaseName::bytes, bytes as f64);
    }
    let (outcome, fail) = match outcome {
        FetchOutcome::Read => match adopt(category, subject, block, &mut phases) {
            Ok(()) => (Outcome::ok, Failure::default()),
            Err(fail) => (Outcome::error, fail),
        },
        // There is no such object, and for most categories that is an answer
        // rather than a fault — which of the two is the category's own and
        // is the one piece of this the world decides.
        FetchOutcome::Absent => {
            release(block);
            match absent(category, subject) {
                Ok(()) => (Outcome::ok, Failure::default()),
                Err(fail) => (Outcome::error, fail.into()),
            }
        }
        FetchOutcome::Aborted => {
            release(block);
            (Outcome::aborted, Failure::default())
        }
        // The host's own failure. A failure that names none is still one the
        // host raised, not a read that went well.
        FetchOutcome::Error => {
            release(block);
            (Outcome::error, if fail.fail == FetchFail::None { Failure { fail: FetchFail::Shell, ..fail } } else { fail })
        }
    };
    settle(key, outcome, Some(phases), fail, policy::retry_after(outcome));
}

/// There is no such object. What that means is the category's: open water
/// for a terrain page, no label for a geocode group. POI snapshots must exist.
/// The shared ocean tile and the dictionary are the two that must exist, so
/// their absence is a fault.
fn absent(category: Category, subject: u64) -> Result<(), FetchFail> {
    match category {
        Category::terrain => Terrain::absent(LevelPage::of_subject(subject)),
        Category::poi => PoiPage::absent(LevelPage::of_subject(subject)),
        Category::geocode => geocode_store::set_group(subject, None),
        // A bake with no dictionary is one whose groups name nothing: an
        // answer, so the demand is not made again.
        Category::geocode_dict => geocode_store::set_dict(None),
        Category::ocean => return Err(FetchFail::Refused),
        // An asset's read is settled where it is asked for (io/load.rs) and
        // never arrives here.
        Category::asset => return Err(FetchFail::Shell),
    };
    Ok(())
}

/// The bytes arrived. The block is released here, unless the page keeps it.
fn adopt(category: Category, subject: u64, block: Option<Block>, phases: &mut Phases) -> Result<(), Failure> {
    // A read that answered with nothing is not an object; only `absent` says
    // there is none, and it carries no body.
    let Some(b) = block else { return Err(FetchFail::Decode.into()) };
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
        Category::asset => Err(FetchFail::Shell.into()),
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
