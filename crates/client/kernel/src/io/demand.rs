// pxl-kernel/io/demand.rs

//! One deferrable read for the request scheduler (io/sched.rs). The scheduler
//! orders and cancels; the shell does the transport.

use pxl_spec::bake::published::BakeKind;

pub use crate::sched::profiler::spec::TaskPhaseName;
use core::cmp::Ordering;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::aabb::Aabb;

pub const PHASES_MAX: usize = 6;

/// Sub-timings a run reports (e.g. network vs decode), named by the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phases {
    items: [(TaskPhaseName, f64); PHASES_MAX],
    len: usize,
}

impl Default for Phases {
    fn default() -> Phases {
        Phases {
            items: [(TaskPhaseName::net_ms, 0.0); PHASES_MAX],
            len: 0,
        }
    }
}

impl Phases {
    /// Past PHASES_MAX a timing is left out.
    pub fn push(&mut self, name: TaskPhaseName, value: f64) {
        if self.len < PHASES_MAX {
            self.items[self.len] = (name, value);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (TaskPhaseName, f64)> + '_ {
        self.items[..self.len].iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A stream of demands; the profiler's CATEGORY_NAMES. An asset shares the
/// tiles' credits but is never withdrawn: it is wanted until it arrives.
pub use crate::sched::profiler::spec::CategoryName as Category;

impl Category {
    /// An asset belongs to no bake and is asked for by its own command.
    pub fn bake(self) -> Option<BakeKind> {
        match self {
            Category::terrain => Some(BakeKind::terrain),
            Category::poi => Some(BakeKind::poi),
            Category::earth => Some(BakeKind::earth),
            Category::geocode | Category::geocode_dict => Some(BakeKind::geocode),
            Category::asset => None,
        }
    }
}

/// `subject` is a tile's coordinate, or nothing for a stream of one.
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

/// Why a tile is asked for, strongest first: the hold residency keeps it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Why {
    Floor,
    /// On the line over a destination, which the camera waits for exactly.
    Destination,
    /// On the line over a miss, so a coarser answer is held wherever the view
    /// looks and leaving a zoom shows its parent at once (Cesium's `preloadAncestors`).
    Ancestor,
    Demand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ask {
    pub why: Why,
    pub order: Order,
}

impl Ask {
    /// A want with no place in the view, ordered by its category alone.
    pub const PLAIN: Ask = Ask { why: Why::Demand, order: Order::LAST };

    /// The stronger reason and the earlier place, taken separately: how a tile
    /// is held says nothing of when the view needs it.
    pub fn best(self, other: Ask) -> Ask {
        Ask {
            why: self.why.min(other.why),
            order: self.order.min(other.order),
        }
    }
}

/// Seen before unseen, then nearer first. Every layer's work is taken in
/// this order, and eviction in reverse.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    pub near: f64,
}

impl Order {
    pub const LAST: Order = Order { seen: false, near: f64::INFINITY };

    pub const FIRST: Order = Order { seen: true, near: 0.0 };

    pub fn of(o: &Observation, b: &Aabb) -> Order {
        let m = o.measure(b);
        Order { seen: m.seen, near: m.near }
    }

}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        other.seen.cmp(&self.seen).then(self.near.total_cmp(&other.near))
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Order {
    fn eq(&self, other: &Order) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Order {}
