// pxl-kernel/layer/query.rs

//! The query: the only place a layer's storage shape is known
//! (decision/20260928-what-is-drawn-is-what-the-screen-covers.md, "The
//! query"). It is asked of a node of the one hierarchy, a cube by its zoom
//! and index, the same of every layer; how a layer answers is its own. It
//! knows nothing of the view: what a chunk is, and how fine its cells, are
//! said by the asker. Neither form fetches, holds or asks for anything: an
//! answer is exact, coarser with the tile it lacked, or nothing and that tile.
//!
//! The decision's two forms are these two methods' readers: the first hit
//! along a direction is a descent over `bound`, and a value at a point is
//! `read` of the cell there. No third query is written beside them. A
//! descent carries a cursor from each node to its children, so a step reads
//! only what is new there; a node asked alone is the same descent folded to
//! it (`bounds`).

use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use super::answer::Answer;

/// What a node holds, as far as a descent needs: a box holding what is
/// stored there, and the finest zoom any of it is held at, read no finer than
/// one past the cell asked — all a reader of cells can tell apart is whether
/// something is finer than them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub aabb: Aabb,
    pub finest: Zoom,
}

impl Bound {
    /// What two hold together: both boxes, the finer of their finest.
    pub fn join(a: Option<Bound>, b: Option<Bound>) -> Option<Bound> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Bound {
                aabb: a.aabb.join(b.aabb),
                finest: a.finest.max(b.finest),
            }),
            (a, b) => a.or(b),
        }
    }
}

/// A layer's answers, by the node asked.
pub trait Query {
    /// A node's cells at the zoom asked: a view over what holds them, never
    /// a copy, so reading one only for what answered costs nothing; it names
    /// what answered, which a build is keyed by.
    type Record<'a>
    where
        Self: 'a;

    /// What a descent carries from a node to its children.
    type Cursor: Copy;

    /// `node`'s cursor asked alone: the layer's descent folded to it.
    fn cursor<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Self::Cursor;

    /// A child's cursor from its parent's; `node` is the child.
    fn child<const D: usize>(&self, parent: &Self::Cursor, node: Node<D>, cell: Zoom) -> Self::Cursor;

    /// Where anything stands in the node, as its cells at `cell` are read,
    /// off its cursor: none where nothing does. What is not known is the
    /// answer's, not a bound's: no third state.
    /// A node in fewer than three axes is the column over the rest.
    fn bound<const D: usize>(&self, at: &Self::Cursor, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>>;

    fn bounds<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        self.bound(&self.cursor(node, cell), node, cell)
    }

    /// The node's cells at `cell`, no coarser than the node.
    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Self::Record<'_>>;
}
