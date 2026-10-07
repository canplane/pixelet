// pxl-kernel/view/frame.rs

//! The frame the world is drawn in: the observer's pose and its matrices,
//! the world's doubles a canonical point enters on the lap nearest the eye,
//! and the ratio a surface is drawn at. No origin is kept: a draw is placed
//! less the eye, in doubles, before it is narrowed.

pub mod coord;
pub mod pose;
pub mod resolution;
