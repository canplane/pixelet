// pxl_core/module/world/change.rs

//! The world's change log: which units of world-shared state changed, read by
//! whoever cares, the way issue/20260913-the-read-direction-a-change-log.md
//! describes. The log is the kernel's (xpute_core::ipc::change), where the
//! state that changes most — the object store — bumps it itself; this is its
//! face for the modules: the topics, a bump, and the frame a bump asks for.
//!
//! A source calls `bump_world`. Readers compare revisions or walk the
//! journal. The canvas renders on demand, so a bump also asks for that frame;
//! the request is taken by the tick driver, which is the one thing that knows
//! a canvas exists.

use xpute_core::ipc::change::{ChangeLog, Revision, Topic};

use crate::global::Global;
use crate::kernel::kernel;

/// A column's cube content (object/store.rs), keyed by column base.
pub const OBJECT_COLUMN: Topic = 0;
/// A column's placed decor, apart from its cubes, keyed by column base.
pub const DECOR_COLUMN: Topic = 1;
/// One cell's placed entries, keyed by cell corner.
pub const OBJECT_CELL: Topic = 2;
/// A resident terrain page arrived or left, keyed by page index.
pub const TERRAIN_PAGE: Topic = 3;
/// A resident POI page arrived or left, keyed by page index.
pub const POI_PAGE: Topic = 4;
/// A decor model or skin finished loading, keyed by its URL's FNV-1a hash.
pub const DECOR_ASSET: Topic = 5;

/// `WorldTopic`.
pub type WorldTopic = Topic;

/// The kernel's log.
pub fn world_changes() -> &'static mut ChangeLog {
    &mut kernel().changes
}

static TICK_REQUESTED: Global<bool> = Global::new();

/// Asks for the frame that delivers changes made in the kernel — the store's
/// own bumps — the way bump_world does for a change made here.
pub fn request_world_tick() {
    TICK_REQUESTED.set(true);
}

/// Whether a frame was asked for since the last take: the tick driver's.
pub fn take_tick_request() -> bool {
    let requested = *TICK_REQUESTED.get(|| false);
    TICK_REQUESTED.set(false);
    requested
}

/// Records a change to world state and asks for the frame that delivers it.
pub fn bump_world(topic: WorldTopic, key: u64) -> Revision {
    let revision = world_changes().bump(topic, key as i64);
    request_world_tick();
    revision
}
