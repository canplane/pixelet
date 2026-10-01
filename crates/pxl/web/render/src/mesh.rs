// pxl-render/mesh.rs

//! A chunk's voxels to triangles: the volume a chunk's cells live in, the
//! scan that turns its bands into column runs, and the mesher that turns
//! those runs into one mesh. Minecraft's own chunk model, for the same
//! reason — a face between two solid cells is never drawn.

pub mod chunk_mesher;
pub mod paint;
pub mod volume;
pub mod voxel_scan;

// What a chunk's build works in grows with what stands in the chunk: the
// bands it reaches, the runs a column holds, the sub-cell objects on a
// voxel, the faces that survive. None of it has a bound the world gives, so
// it is the heap's, and a build the heap refuses room is a build that waits
// — it ends, what is drawn stays drawn, and it is asked for again once the
// heap has let something go. Growth goes through these, which ask for room
// before a vector reallocates and say so when it is refused, rather than
// letting the allocator's failure end the module.

/// The heap refused a build's workspace room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

impl From<std::collections::TryReserveError> for Refused {
    fn from(_: std::collections::TryReserveError) -> Refused {
        Refused
    }
}

/// Room in `v` for `more` past what it holds, doubling as a vector does.
pub(crate) fn grow<T>(v: &mut Vec<T>, more: usize) -> Result<(), Refused> {
    let need = v.len() + more;
    if need > v.capacity() {
        v.try_reserve(need.max(2 * v.len()) - v.len())?;
    }
    Ok(())
}

/// `x` pushed onto `v`, where the heap has room for it.
pub(crate) fn try_push<T>(v: &mut Vec<T>, x: T) -> Result<(), Refused> {
    grow(v, 1)?;
    v.push(x);
    Ok(())
}
