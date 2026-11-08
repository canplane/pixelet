// pxl-core/domain/layer/query.rs

//! The query that the terrain and the object store both answer. It is the
//! only way the rest of the core reads either layer, so neither layer's
//! storage shape is known outside it. A query names a node of the hierarchy
//! the two share, a cube given by its zoom and index, and each layer answers
//! in its own way. Places, divisions and the Earth answer no query.
//!
//! The query knows nothing of the view: the asker says which node it wants
//! and how fine its cells are. Answering fetches, holds and asks for nothing.
//! An answer is exact, coarser with the tile it lacked, or nothing with that
//! tile.
//!
//! The world's two questions are built on these methods: the first hit along
//! a direction is a descent over `bound`, and the value at a point is the
//! asset in the world's cell there (domain/query/node.rs `World::asset`). A
//! descent carries a cursor from each node to its children, so each step
//! reads only what is new at that node. A node asked alone is the same
//! descent folded into one call (`bounds`).

use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use super::answer::Answer;

/// A layer's answers, by the node asked. Only the world's query
/// (domain/query.rs) reads it; every other reader asks the world.
pub(in crate::domain) trait Query {
    /// What a descent carries from a node to its children.
    type Cursor: Copy;

    /// The cursor of `node` asked alone. It equals what the layer's descent
    /// from the top would carry to `node`.
    fn cursor<const D: usize>(&self, node: Node<D>, cell_zoom: Zoom) -> Self::Cursor;

    /// A child's cursor from its parent's; `node` is the child.
    fn child<const D: usize>(
        &self,
        parent: &Self::Cursor,
        node: Node<D>,
        cell_zoom: Zoom,
    ) -> Self::Cursor;

    /// A box around everything that may show in `node` when its cells are
    /// read at `cell_zoom`, computed from the node's cursor, or none where
    /// nothing shows. The box covers what is stored down to the depth where
    /// everything around it is solid, since nothing past that depth is seen.
    /// What is not known is carried by the `Answer`, not by the box, so a
    /// bound has no third state. A node with fewer than three axes stands for
    /// the whole column over the axes it lacks.
    fn bound<const D: usize>(
        &self,
        at: &Self::Cursor,
        node: Node<D>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>>;

    #[cfg(test)]
    fn bounds<const D: usize>(&self, node: Node<D>, cell_zoom: Zoom) -> Answer<Option<Aabb>> {
        self.bound(&self.cursor(node, cell_zoom), node, cell_zoom)
    }
}
