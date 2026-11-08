// pxl-core/domain/control/navigation.rs

//! Where the camera goes: to a named place (`destination`), after the
//! device (`follow_mode`, `off_center`), or to the fallback start when no
//! location comes (`bootstrap`).

pub mod bootstrap;
pub mod destination;
pub mod follow_mode;
pub mod off_center;
