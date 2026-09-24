// pxl-kernel/io/demand.rs

//! One unit of deferrable I/O handed to scheduler.rs.
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

use pxl_spec::bake::published::BakeKind;

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
    /// One group of pages of the reverse-geocode bake.
    Geocode,
    /// Its dictionary, which is a stream of one.
    GeocodeDict,
    /// A model or an image from the catalog. It shares the credits with the
    /// pages and nothing else: a page is wanted while the view is where it
    /// is, and an asset is wanted from the first time something needs it
    /// until it is there, so it is never stale and never withdrawn.
    Asset,
}

impl Category {
    /// Which bake an object of this category belongs to. The name inside that
    /// bake is the layer's (`DemandName`); where the two together are read
    /// from is the host's, and nothing here knows it.
    /// Which bake an object of this category belongs to, for the categories
    /// whose objects are pages. An asset belongs to no bake — it is named by
    /// its content — and is asked for by its own command.
    pub fn bake(self) -> Option<BakeKind> {
        match self {
            Category::Terrain | Category::Ocean => Some(BakeKind::terrain),
            Category::Poi => Some(BakeKind::poi),
            Category::Geocode | Category::GeocodeDict => Some(BakeKind::geocode),
            Category::Asset => None,
        }
    }

    /// A short name for a report to print, as `Outcome::as_str` is.
    pub const fn as_str(self) -> &'static str {
        match self {
            Category::Terrain => "terrain",
            Category::Ocean => "ocean",
            Category::Poi => "poi",
            Category::Geocode => "geocode",
            Category::GeocodeDict => "geocode_dict",
            Category::Asset => "asset",
        }
    }
}

/// What names one demand: the stream it belongs to, and the one thing in that
/// stream it is for.
///
/// The subject is a page's own coordinate where the stream is pages, and
/// nothing where the stream has one member.
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
    /// Lower runs sooner. A page's is how far it sits from the view in its
    /// category's own order (terrain/tile_fetcher.rs's), doubled so a second
    /// category can interleave at the odd numbers.
    pub priority: f64,
}
