// pxl-core/domain/view/frame.rs

//! The frame the world is drawn in: the observer's pose and its matrices,
//! and positions in the world's doubles. X wraps around the world, and a
//! canonical point enters the doubles on the lap of X nearest the eye. The
//! frame keeps no origin of its own: each draw subtracts the eye in doubles
//! before it is narrowed to f32.

pub(crate) mod coord;
pub(crate) mod pose;
pub(crate) mod screen;
