// pxl-core/domain/layer/answer.rs

//! A layer query's answer: a value read from the exact tile, or from a
//! coarser one together with the tile that was lacking, or a lack, no value,
//! with that tile.

use pxl_spec::space::layer::quadrant::Quadrant;

/// A coarser answer, and a lack, which gives no value, both carry the tile
/// that was lacking, so a coarser answer can never pass for an exact one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Answer<T> {
    Exact(T),
    Coarser(T, Quadrant),
    Lack(Quadrant),
}

impl<T> Answer<T> {
    pub(crate) fn val(self) -> Option<T> {
        match self {
            Answer::Exact(v) | Answer::Coarser(v, _) => Some(v),
            Answer::Lack(_) => None,
        }
    }

    pub(crate) fn exact(self) -> Option<T> {
        match self {
            Answer::Exact(v) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn lack(&self) -> Option<Quadrant> {
        match self {
            Answer::Exact(_) => None,
            Answer::Coarser(_, l) | Answer::Lack(l) => Some(*l),
        }
    }

    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        match self {
            Answer::Exact(v) => Answer::Exact(f(v)),
            Answer::Coarser(v, l) => Answer::Coarser(f(v), l),
            Answer::Lack(l) => Answer::Lack(l),
        }
    }
}
