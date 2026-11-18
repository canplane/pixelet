// pxl-core/sys/io/demand.rs

//! What a request is for: the stream it belongs to (demand.json5
//! `STREAM_NAME`), which one row of a table describes (`Stream::row`), and
//! the order of its want in the view. The timings a run reports are here
//! too.

use core::cmp::Ordering;

use pxl_spec::bake::publication::BaseKind;

pub(crate) use crate::spec::demand::StreamName as Stream;
pub(crate) use crate::spec::demand::TaskPhaseName;

/// Where a stream's wants come from. A `Load` want is an asset file by its
/// id, which the loader opens (sys/io/loader.rs). A `Base` want is a base
/// layer's tile by its quadrant. An `Object` want is a shard's objects or the
/// object top.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Load,
    Base(BaseKind),
    Object,
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
    /// Whether the stream's bodies take a body slot (sys/io/body.rs).
    pub slotted: bool,
    pub class: Class,
}

/// A want's order in the view: seen before unseen, then nearer first. Every
/// layer takes its work in this order and evicts it in reverse.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Order {
    pub seen: bool,
    pub near: f64,
}

pub(crate) const PHASE_CAP: usize = TaskPhaseName::ALL.len();

/// The timings a run reports, each under the name the run gives it and each
/// at most once, in the order it reports them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PhaseList {
    items: [(TaskPhaseName, f64); PHASE_CAP],
    len: usize,
}

impl Stream {
    /// The one table of what each stream is.
    pub const fn row(self) -> Row {
        let (source, slotted, class) = match self {
            Stream::Terrain => (Source::Base(BaseKind::Terrain), true, Class::Ground),
            Stream::Place => (Source::Base(BaseKind::Place), false, Class::Place),
            Stream::Division => (Source::Base(BaseKind::Division), false, Class::Aside),
            Stream::Asset => (Source::Load, false, Class::File),
            Stream::Imagery => (Source::Base(BaseKind::Imagery), true, Class::Picture),
            Stream::Object => (Source::Object, false, Class::OnGround),
        };
        Row {
            source,
            slotted,
            class,
        }
    }

    pub const fn layer(self) -> Option<BaseKind> {
        match self.row().source {
            Source::Base(layer) => Some(layer),
            Source::Load | Source::Object => None,
        }
    }

    /// The most bytes one of the stream's bodies takes: the longest packet of
    /// its kind (pxl-spec `BaseKind::row`). A load and a shard's objects
    /// have none, because only the heap bounds them.
    pub const fn max_body(self) -> Option<u32> {
        match self.layer() {
            Some(layer) => Some(layer.row().pkt_cap),
            None => None,
        }
    }

    /// The stream `layer`'s tiles come in, the one row that names it.
    pub const fn of_layer(layer: BaseKind) -> Stream {
        let mut s = 0;
        while s < Stream::ALL.len() {
            if let Source::Base(l) = Stream::ALL[s].row().source
                && l as u32 == layer as u32
            {
                return Stream::ALL[s];
            }
            s += 1;
        }
        panic!("a base layer is one stream's")
    }
}

impl Order {
    /// A want with no place in the view, ordered by its stream alone.
    pub(crate) const LAST: Order = Order {
        seen: false,
        near: f64::INFINITY,
    };

    pub(crate) const FIRST: Order = Order {
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
            items: [(TaskPhaseName::NetMs, 0.0); PHASE_CAP],
            len: 0,
        }
    }
}

impl PhaseList {
    pub(crate) fn push(&mut self, name: TaskPhaseName, val: f64) {
        self.items[self.len] = (name, val);
        self.len += 1;
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (TaskPhaseName, f64)> + '_ {
        self.items[..self.len].iter().copied()
    }
}
