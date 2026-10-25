// pxl-spec/block.rs

//! A block: an instance standing in a cell of the world's grid. `object`
//! is the value the object layer keeps for one, the id of the asset it is
//! (`asset`); `ext` what a placed one carries beyond that id; `volume` the
//! objects' tree folded as it is written; `limit` where the world ends.

pub mod ext;
pub mod limit;
pub mod object;
pub mod volume;
