// pxl-render/look.rs

//! How the world is meant to look, as numbers a frame can use: a theme's
//! colours, the light and fog and sky it is lit by, and the tier a thing
//! shows at a distance. The themes are generated from spec/; what is here is
//! what reading one comes to.

pub mod color;
pub mod environment;
pub mod lod;
pub mod style;
