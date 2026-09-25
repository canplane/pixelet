// pxl-kernel/layer/object.rs

//! The cubes a person places: where they are, what they are made of, and how
//! large they may be; what each paints as is the renderer's (pxl-render
//! mesh/paint/object/material.rs). The
//! store is both the record and the spatial index, because a placement has
//! to answer "what is already here" before it is allowed.

pub mod collision;
pub mod geometry;
pub mod size;
pub mod store;
