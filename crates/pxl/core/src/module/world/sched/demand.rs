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

use core::fmt::{self, Write};


/// The most sub-timings one run reports.
pub const PHASES_MAX: usize = 6;

/// Optional sub-timings a demand's own run can hand back — terrain splitting
/// its network wait from its decode, say. Lands in TaskRecord.phases, so the
/// report can state whether a category is actually network- or memory-bound
/// instead of leaning on category.rs's static guess. Named by the run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Phases {
    items: [(&'static str, f64); PHASES_MAX],
    len: usize,
}

impl Phases {
    /// One more timing; past PHASES_MAX it is left out.
    pub fn push(&mut self, name: &'static str, value: f64) {
        if self.len < PHASES_MAX {
            self.items[self.len] = (name, value);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, f64)> + '_ {
        self.items[..self.len].iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A demand's key: `<category>:<subject>` — a page, or what stands for one.
pub type DemandKey = String;

/// Why a task failed, as the ledger keeps it: 80 characters at most.
pub type ErrorText = String;

/// `<category>:<subject>`. The subjects are pages and hashes, well inside a key.
pub fn demand_key(category: &str, subject: impl fmt::Display) -> DemandKey {
    let mut key = DemandKey::new();
    let _ = write!(key, "{category}:{subject}");
    key
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Demand {
    /// Stable identity across sync calls. Two demands sharing a key are the
    /// same work: re-submitting one that is already queued updates its
    /// priority, one already in flight is left alone.
    pub key: DemandKey,
    /// Coarse grouping — scopes sync's diff, and is what map_status and the
    /// ledger roll up by.
    pub category: &'static str,
    /// Lower runs sooner. Convention: the page's Euclidean distance from the
    /// view anchor in whole pages (range.rs's page_priority), so the nearest
    /// page loads first.
    pub priority: f64,
}
