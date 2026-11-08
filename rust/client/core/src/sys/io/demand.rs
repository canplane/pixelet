// pxl-core/sys/io/demand.rs

//! What a request is for: its stream (demand.json `STREAM_NAMES`), each as
//! one row says, and its want's order in the view; and the timings its run
//! reports.

use core::cmp::Ordering;

use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile;

use crate::sys::io::demand_spec;
pub use crate::sys::io::demand_spec::StreamName as Stream;
pub use crate::sys::io::demand_spec::TaskPhaseName;
use crate::sys::io::file_spec::root;

/// What a stream's wants are: a file of the catalog's by its id, which the
/// loader opens below ASSET (sys/io/loader.rs); a bake's tile by its
/// quadrant, below its kind's root; or a shard's objects or their top, which
/// no bake makes, below OBJECT.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Load,
    Bake(BaseLayer),
    Objects,
}

/// Where a stream's wants stand in the order a view builds up, as the
/// policy ranks them (sys/io/policy.rs).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The ground's shells, seen then unseen, the finer first.
    Ground,
    /// What stands on the ground, a shell's after the shell's ground.
    OnGround,
    /// With the pictures, seen then unseen, the finer first.
    Picture,
    /// After the models, seen then unseen, the finer first.
    Place,
    /// A file of the catalog's: a picture's with the pictures, a model's
    /// after them.
    File,
    /// After everything, as it comes.
    Aside,
}

#[derive(Clone, Copy)]
pub struct Row {
    pub source: Source,
    /// Whether its bodies take one of the body slots (sys/io/body.rs).
    pub slotted: bool,
    pub class: Class,
}

/// Seen before unseen, then nearer first: every layer's work is taken in
/// this order, and evicted in reverse.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    pub near: f64,
}

pub const PHASES_MAX: usize = demand_spec::TASK_PHASES_MAX as usize;

/// The timings a run reports, each by the name its run gives it; one past
/// `PHASES_MAX` is left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phases {
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

    /// The root its wants' files are opened below (file.json `root`), a
    /// want's id the path; none for a load, which the loader opens.
    pub const fn root(self) -> Option<u32> {
        match self.row().source {
            Source::Bake(layer) => Some(layer as u32),
            Source::Objects => Some(root::OBJECT),
            Source::Load => None,
        }
    }

    /// The most bytes a body of its takes, its kind's packet at its longest
    /// (pxl-spec `bake::tile::max_pkt_bytes`); none for a load or a shard's
    /// objects, which the heap alone bounds.
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

impl Default for Phases {
    fn default() -> Phases {
        Phases {
            items: [(TaskPhaseName::net_ms, 0.0); PHASES_MAX],
            len: 0,
        }
    }
}

impl Phases {
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
