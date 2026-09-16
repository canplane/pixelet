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

use core::fmt;



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

/// Which stream of deferrable I/O a demand belongs to. Coarse grouping —
/// scopes sync's diff, and is what map_status and the ledger roll up by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Terrain,
    /// The one tile every open-water page shares.
    Ocean,
    Poi,
    Search,
    Geocode,
}

impl Category {
    /// A short name for a report to print, as `Outcome::as_str` is.
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Terrain => "terrain",
            Category::Ocean => "ocean",
            Category::Poi => "poi",
            Category::Search => "search",
            Category::Geocode => "geocode",
        }
    }
}

/// What names one demand: the stream it belongs to, and the one thing in that
/// stream it is for.
///
/// The subject is a page's own coordinate where the stream is pages, the
/// query's hash where it is a search, and nothing where the stream has one
/// member. It used to be `"<category>:<subject>"` spelled out, which the
/// arrival then split back apart and parsed — two numbers folded into text so
/// that the text could be unfolded into two numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DemandKey {
    pub category: Category,
    pub subject: u64,
}

impl DemandKey {
    pub fn new(category: Category, subject: u64) -> DemandKey {
        DemandKey { category, subject }
    }
}

impl fmt::Display for DemandKey {
    /// `terrain:3231700000000`, for a report to print.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.category.as_str(), self.subject)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Demand {
    /// Stable identity across sync calls. Two demands sharing a key are the
    /// same work: re-submitting one that is already queued updates its
    /// priority, one already in flight is left alone.
    pub key: DemandKey,
    /// Lower runs sooner. Convention: the page's Euclidean distance from the
    /// view anchor in whole pages (range.rs's page_priority), so the nearest
    /// page loads first.
    pub priority: f64,
}
