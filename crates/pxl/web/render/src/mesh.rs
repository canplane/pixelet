// pxl-render/mesh.rs

//! A chunk's voxels to triangles: the volume a chunk's cells live in, the
//! scan that turns its bands into column runs, and the mesher that turns
//! those runs into one mesh. Minecraft's own chunk model, for the same
//! reason — a face between two solid cells is never drawn.

pub mod chunk_mesher;
pub mod paint;
pub mod volume;
pub mod voxel_scan;
