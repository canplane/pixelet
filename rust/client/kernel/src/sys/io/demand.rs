// pxl-kernel/sys/io/demand.rs

//! One deferrable read for the request scheduler (sys/io/request.rs). The scheduler
//! orders and cancels; the shell does the transport.

use pxl_spec::bake::published::BaseLayer;

pub use crate::sys::io::demand_spec::TaskPhaseName;
use core::cmp::Ordering;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::aabb::Aabb;

pub const PHASES_MAX: usize = crate::sys::io::demand_spec::TASK_PHASES_MAX as usize;

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
}

/// A stream of demands (spec `STREAM_NAMES`). An asset shares the
/// tiles' credits but is never withdrawn: it is wanted until it arrives.
pub use crate::sys::io::demand_spec::StreamName as Stream;

impl Stream {
    /// An asset belongs to no bake and is asked for by its own command.
    pub const fn bake(self) -> Option<BaseLayer> {
        match self {
            Stream::terrain => Some(BaseLayer::terrain),
            Stream::place => Some(BaseLayer::place),
            Stream::earth => Some(BaseLayer::earth),
            Stream::division => Some(BaseLayer::division),
            Stream::asset => None,
        }
    }

    /// The most bytes a body of its takes, its kind's packet at its longest
    /// (pxl-spec `bake::tile::max_packet_bytes`): one past it is refused
    /// before the heap is asked (sys/io/fetch.rs `on_opened`). None for an asset,
    /// which the heap alone bounds.
    pub const fn max_body(self) -> Option<u32> {
        match self.bake() {
            Some(layer) => Some(pxl_spec::bake::tile::max_packet_bytes(layer) as u32),
            None => None,
        }
    }
}

/// Why a tile is asked for, strongest first: the hold residency keeps it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Why {
    Floor,
    /// On a line asked for exactly: the camera's destination's, or the division layer's focus's.
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
    /// A want with no place in the view, ordered by its stream alone.
    pub const PLAIN: Ask = Ask {
        why: Why::Demand,
        order: Order::LAST,
    };

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
    pub const LAST: Order = Order {
        seen: false,
        near: f64::INFINITY,
    };

    pub const FIRST: Order = Order {
        seen: true,
        near: 0.0,
    };

    pub fn of(o: &Observation, b: &Aabb) -> Order {
        let m = o.measure(b);
        Order {
            seen: m.seen,
            near: m.near,
        }
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        other
            .seen
            .cmp(&self.seen)
            .then(self.near.total_cmp(&other.near))
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
