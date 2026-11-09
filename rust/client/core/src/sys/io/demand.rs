// pxl-core/sys/io/demand.rs

//! What a request is for: the stream it belongs to (demand.json
//! `STREAM_NAMES`), which one row of a table describes (`Stream::row`), and
//! the order of its want in the view. The timings a run reports are here
//! too.

use core::cmp::Ordering;

use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile;

use crate::sys::io::demand_spec;
pub use crate::sys::io::demand_spec::StreamName as Stream;
pub use crate::sys::io::demand_spec::TaskPhaseName;
use crate::sys::io::file_spec::root;

/// Where a stream's wants come from. A `Load` want is an asset file by its
/// id, which the loader opens (sys/io/loader.rs). A `Bake` want is a bake's
/// tile by its quadrant, below the root of the bake's kind. An `Objects` want
/// is a shard's objects or the objects' top, which no bake makes, below
/// OBJECT.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Load,
    Bake(BaseLayer),
    Objects,
}

/// Where a stream's wants stand in the order a view builds up. The policy
/// ranks the classes (sys/io/policy.rs).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The ground: seen then unseen, finer tiles first.
    Ground,
    /// What stands on the ground. Each shell of it ranks after the ground of
    /// the same shell.
    OnGround,
    /// Ranks with the pictures: seen then unseen, finer tiles first.
    Picture,
    /// Ranks after the models: seen then unseen, finer tiles first.
    Place,
    /// A file of the catalog. A picture ranks with the pictures, and a model
    /// after them.
    File,
    /// Ranks after everything else, with no order among its wants.
    Aside,
}

#[derive(Clone, Copy)]
pub struct Row {
    pub source: Source,
    /// Whether the stream's bodies take a body slot, each no longer than a
    /// slot (sys/io/body.rs).
    pub slotted: bool,
    pub class: Class,
}

/// A want's order in the view: seen before unseen, then nearer first. Every
/// layer takes its work in this order and evicts it in reverse.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    pub near: f64,
}

pub const PHASES_MAX: usize = demand_spec::TASK_PHASES_MAX as usize;

/// The timings a run reports, each under the name the run gives it. A timing
/// past `PHASES_MAX` is dropped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseList {
    items: [(TaskPhaseName, f64); PHASES_MAX],
    len: usize,
}

impl Stream {
    /// The one table of what each stream is.
    pub const fn row(self) -> Row {
        let (source, slotted, class) = match self {
            Stream::terrain => (Source::Bake(BaseLayer::terrain), true, Class::Ground),
            Stream::place => (Source::Bake(BaseLayer::place), false, Class::Place),
            Stream::division => (Source::Bake(BaseLayer::division), false, Class::Aside),
            Stream::asset => (Source::Load, false, Class::File),
            Stream::earth => (Source::Bake(BaseLayer::earth), true, Class::Picture),
            Stream::object => (Source::Objects, false, Class::OnGround),
        };
        Row {
            source,
            slotted,
            class,
        }
    }

    pub const fn bake(self) -> Option<BaseLayer> {
        match self.row().source {
            Source::Bake(layer) => Some(layer),
            Source::Load | Source::Objects => None,
        }
    }

    /// The root that the stream's files are opened below (file.json `root`),
    /// with a want's id as the path. A load has none, because the loader
    /// chooses its root.
    pub const fn root(self) -> Option<u32> {
        match self.row().source {
            Source::Bake(layer) => Some(layer as u32),
            Source::Objects => Some(root::OBJECT),
            Source::Load => None,
        }
    }

    /// The most bytes one of the stream's bodies takes: the longest packet of
    /// its kind (pxl-spec `bake::tile::max_pkt_bytes`). A load and a shard's
    /// objects have none, because only the heap bounds them.
    pub const fn max_body(self) -> Option<u32> {
        match self.bake() {
            Some(layer) => Some(tile::max_pkt_bytes(layer) as u32),
            None => None,
        }
    }
}

impl Order {
    /// A want with no place in the view, ordered by its stream alone.
    pub const LAST: Order = Order {
        seen: false,
        near: f64::INFINITY,
    };

    pub const FIRST: Order = Order {
        seen: true,
        near: 0.0,
    };
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

impl Default for PhaseList {
    fn default() -> PhaseList {
        PhaseList {
            items: [(TaskPhaseName::net_ms, 0.0); PHASES_MAX],
            len: 0,
        }
    }
}

impl PhaseList {
    pub fn push(&mut self, name: TaskPhaseName, val: f64) {
        if self.len < PHASES_MAX {
            self.items[self.len] = (name, val);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (TaskPhaseName, f64)> + '_ {
        self.items[..self.len].iter().copied()
    }
}
