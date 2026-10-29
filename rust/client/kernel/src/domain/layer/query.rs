// pxl-kernel/domain/layer/query.rs

//! The query: the form the terrain and the object store share, the only
//! place either's storage shape is known
//! (doc/minutes/archive/20260928-what-is-drawn-is-what-the-screen-covers.md, "The
//! query"). It is asked of a node of the one hierarchy, a cube by its zoom
//! and index, the same of both; how each answers is its own, and each is
//! read as itself: places, divisions and the Earth answer none. It
//! knows nothing of the view: what node is asked, and how fine its cells, are
//! said by the asker. Neither form fetches, holds or asks for anything: an
//! answer is exact, coarser with the tile it lacked, or nothing and that tile.
//!
//! The decision's two forms are these two methods' readers: the first hit
//! along a direction is a descent over `bound`, and a value at a point is
//! the asset in the world's cell there (domain/query/node.rs `World::asset`). No third query
//! is written beside them. A
//! descent carries a cursor from each node to its children, so a step reads
//! only what is new there; a node asked alone is the same descent folded to
//! it (`bounds`).

use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use super::answer::Answer;

/// The terrain's or the object store's answers, by the node asked.
pub trait Query {
    /// What a descent carries from a node to its children.
    type Cursor: Copy;

    /// `node`'s cursor asked alone: the layer's descent folded to it.
    fn cursor<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Self::Cursor;

    /// A child's cursor from its parent's; `node` is the child.
    fn child<const D: usize>(
        &self,
        parent: &Self::Cursor,
        node: Node<D>,
        cell: Zoom,
    ) -> Self::Cursor;

    /// Where anything stands in the node, as its cells at `cell` are read,
    /// off its cursor: a box holding what may show there, what is stored
    /// down to where everything about it is solid and nothing past that is
    /// seen; none where nothing does. What is not known is the
    /// answer's, not a bound's: no third state.
    /// A node in fewer than three axes is the column over the rest.
    fn bound<const D: usize>(
        &self,
        at: &Self::Cursor,
        node: Node<D>,
        cell: Zoom,
    ) -> Answer<Option<Aabb>>;

    fn bounds<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Answer<Option<Aabb>> {
        self.bound(&self.cursor(node, cell), node, cell)
    }
}
