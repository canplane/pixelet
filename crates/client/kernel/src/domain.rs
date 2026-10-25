// pxl-kernel/domain.rs

//! The world and the eye: two heads over one trunk. The scene, what a frame
//! draws, and control, what moves the eye and asks the query, both stand on
//! the view, the query and the layers, and neither names the other. Every
//! edge points down, and nothing here names the turn, the door or the shell.

pub mod approach;
pub mod chunk;
pub mod control;
pub mod layer;
pub mod query;
pub mod scene;
pub mod view;
