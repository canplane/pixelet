// pxl-core/domain.rs

//! The world and the eye, with no knowledge of the frame, the link or the UI.
//! Two heads stand on one trunk. The scene decides what a frame draws.
//! Control moves the eye and asks the query. Both heads use the view, the
//! query and the layers, and neither names the other. A module here uses
//! only the modules below it in that order.

pub(crate) mod control;
pub(crate) mod layer;
pub(crate) mod param;
pub(crate) mod preview;
pub(crate) mod scene;
pub(crate) mod setting;
pub(crate) mod smooth;
pub(crate) mod view;

// The query is a child of `layer` (layer.rs); every caller names it here.
pub(crate) use layer::query;
