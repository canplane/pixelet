// pxl_core/module/world/sched/demand.rs

//! One unit of deferrable I/O handed to fetch_scheduler.rs.
//!
//! The scheduler owns ordering, the concurrency quota, and cancellation of
//! demands that stop being wanted; the shell owns the actual transport — the
//! scheduler starts a demand by handing its key to the shell, and the shell
//! settles it. The scheduler never reads or writes a store.
//!
//! This is the category-B (I/O) shape of the task contract in
//! issue/20260909-cooperative-scheduler-and-task-ledger.md: a black box the
//! scheduler drives by key/priority, nothing more.

/// Optional sub-timings a demand's own run can hand back — terrain splitting
/// its network wait from its decode, say. Lands in TaskRecord.phases, so the
/// report can state whether a category is actually network- or memory-bound
/// instead of leaning on category.rs's static guess. Free-form keys.
pub type Phases = Vec<(String, f64)>;

#[derive(Clone, Debug, PartialEq)]
pub struct Demand {
    /// Stable identity across sync calls, by convention `<category>:<page>`.
    /// Two demands sharing a key are the same work: re-submitting one that is
    /// already queued updates its priority, one already in flight is left
    /// alone.
    pub key: String,
    /// Coarse grouping — scopes sync's diff, and is what map_status and the
    /// ledger roll up by.
    pub category: String,
    /// Lower runs sooner. Convention: the page's Euclidean distance from the
    /// view anchor in whole pages (range.rs's page_priority), so the nearest
    /// page loads first.
    pub priority: f64,
}
