// pxl-core/domain.rs

//! The world and the eye, with no knowledge of the turn, the door or the UI.
//! Two heads stand on one trunk. The scene decides what a frame draws.
//! Control moves the eye and asks the query. Both heads use the view, the
//! query and the layers, and neither names the other. A module here uses
//! only the modules below it in that order.

/// The enums of the UI's protocol that the domain reads, generated from
/// ui.json. The protocol's code itself belongs to the door
/// (door/cmd.spec.rs), so the domain does not depend on it.
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
