// pxl-kernel/world/pyramid/domain/price.rs

use crate::world::pyramid::chunk::Chunk;
use crate::world::chunk::mesh_slot::POOLS;
use pxl_spec::space::camera::grid::Tau;

/// Mesh slots per pool.
pub trait Price {
    fn piece(&self, c: Chunk, near: f64, tau: Tau) -> [u32; POOLS];
    fn split(&self, c: Chunk) -> [u32; POOLS];
    /// stop counting a step needing `n`
    fn over(&self, n: &[u32; POOLS]) -> bool;
}
