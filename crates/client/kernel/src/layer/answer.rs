// pxl-kernel/layer/answer.rs

//! What a query answers: a value, exact or coarser with the tile it lacked.

use pxl_spec::bake::format::GroundCell;
use pxl_spec::block::cube;
use pxl_spec::space::layer::tile::Tile;

/// A coarser answer carries the tile it lacked, so none passes for exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer<T> {
    Exact(T),
    Coarser(T, Tile),
    Nothing(Tile),
}

impl<T> Answer<T> {
    pub fn value(self) -> Option<T> {
        match self {
            Answer::Exact(v) | Answer::Coarser(v, _) => Some(v),
            Answer::Nothing(_) => None,
        }
    }

    pub fn exact(self) -> Option<T> {
        match self {
            Answer::Exact(v) => Some(v),
            _ => None,
        }
    }

    pub fn lack(&self) -> Option<Tile> {
        match self {
            Answer::Exact(_) => None,
            Answer::Coarser(_, l) | Answer::Nothing(l) => Some(*l),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        match self {
            Answer::Exact(v) => Answer::Exact(f(v)),
            Answer::Coarser(v, l) => Answer::Coarser(f(v), l),
            Answer::Nothing(l) => Answer::Nothing(l),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relief {
    /// Lowest and highest ground, in units; read from a coarser tile this is
    /// wider, never narrower.
    pub extent: (f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// The surface's height as drawn, which a line is measured against
    /// whether or not the cell has ground.
    pub height: f64,
    pub cell: GroundCell,
}

impl Ground {
    /// None where the terrain gives no surface, a cell that is no look.
    pub fn top(&self) -> Option<f64> {
        cube::Ground::of(self.cell).is_some().then_some(self.height)
    }
}
