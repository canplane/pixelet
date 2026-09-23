// pxl-render/mesh/paint.rs

//! What a cell's number looks like once drawn: the terrain's four ground
//! channels, the objects a person placed, and the palette both are coloured
//! from. The catalogs are the spec's (pxl-spec cell/); this is what reading
//! one paints.

pub mod level;
pub mod object;
pub mod palette;
pub mod terrain;
