// pxl-render/mesh.rs

//! A chunk's cells to triangles: the column runs its layers write, and the
//! mesher. A face between two solid cells is never drawn.

pub mod chunk;
pub mod paint;
pub mod run;
pub mod word;
