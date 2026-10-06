// pxl-kernel/world/access/query.rs

//! The query: the only place a layer's storage shape is known
//! (decision/20260928-what-is-drawn-is-what-the-screen-covers.md, "The
//! query"). It is asked of a chunk, a cube of the one hierarchy at any zoom,
//! the same of every layer; how a layer answers is its own. Two things are
//! asked: where anything stands in the cube, for a descent that shrinks its
//! box as it goes, and the cube's cells as drawn, for a build. Neither
//! fetches, holds or asks for anything: an answer is exact, coarser with the
//! tile it lacked, or nothing and that tile.

use pxl_spec::space::coord::v3::Aabb;

use crate::world::access::answer::Answer;
use crate::world::pyramid::chunk::Chunk;

/// A layer's answers, by the chunk asked.
pub trait Query {
    /// The cube's cells as drawn, `CHUNK_STEPS` zooms finer: a view over
    /// what holds them, never a copy, naming what answered, which a build is
    /// keyed by.
    type Record<'a>
    where
        Self: 'a;

    /// Where anything stands in the cube: none where nothing does, else a
    /// box never smaller than what is drawn there. What is not known is the
    /// answer's, not a box's: no third state.
    fn bounds(&self, c: Chunk) -> Answer<Option<Aabb>>;

    fn read(&self, c: Chunk) -> Answer<Self::Record<'_>>;
}
