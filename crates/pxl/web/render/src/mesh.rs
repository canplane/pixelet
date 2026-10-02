// pxl-render/mesh.rs

//! A chunk's voxels to triangles: the volume, the scan to column runs, and the
//! mesher. A face between two solid cells is never drawn.

pub mod chunk_mesher;
pub mod paint;
pub mod volume;
pub mod voxel_scan;

// A build's workspace grows with what stands in the chunk, so it is the heap's.
// Growth asks for room first; a refused build ends, what is drawn stays drawn,
// and it is asked for again later, rather than the allocator ending the module.

/// The heap refused a build's workspace room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

impl From<std::collections::TryReserveError> for Refused {
    fn from(_: std::collections::TryReserveError) -> Refused {
        Refused
    }
}

pub(crate) fn grow<T>(v: &mut Vec<T>, more: usize) -> Result<(), Refused> {
    let need = v.len() + more;
    if need > v.capacity() {
        v.try_reserve(need.max(2 * v.len()) - v.len())?;
    }
    Ok(())
}

pub(crate) fn try_push<T>(v: &mut Vec<T>, x: T) -> Result<(), Refused> {
    grow(v, 1)?;
    v.push(x);
    Ok(())
}
