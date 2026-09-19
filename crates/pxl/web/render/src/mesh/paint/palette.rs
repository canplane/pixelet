// pxl-render/mesh/paint/palette.rs

//! Every colour a cell can be, numbered once: the tables a shader indexes
//! and the array texture they are uploaded as. The object colours follow the
//! ground layers rather than starting over, so one number reaches both.

pub mod shader_meta;
pub mod tex_array;
