// pxl-kernel/domain.rs

//! The world and the eye: two heads over one trunk. The scene, what a frame
//! draws, and control, what moves the eye and asks the query, both stand on
//! the view, the query and the layers, and neither names the other. Every
//! edge points down, and nothing here names the turn, the door or the shell.

/// The shell's protocol's enums the domain reads, generated from shell.json
/// apart from the protocol, which the door's code is (door/cmd.spec.rs).
#[path = "domain/cmd.spec.rs"]
pub mod cmd;
pub mod control;
pub mod layer;
pub mod param;
pub mod preview;
pub mod query;
pub mod scene;
pub mod smooth;
pub mod view;
