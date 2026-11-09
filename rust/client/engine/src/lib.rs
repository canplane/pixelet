// pxl-engine/lib.rs

//! The rules that the CPU runs and the GPU's WGSL is written to. A module headed `wgsl-twin`
//! computes what the CPU and the GPU must compute alike, the WGSL being its other form.
//!
//! The crate holds a node's seed, the answer a node gives, the ground's column and the clip
//! levels that hold its cells, the ways as a cell reads them, what is placed, the ray's
//! descent, and the samples' stops as the boxes a raster draws. It computes over the model
//! pxl-spec gives every crate (the zooms, the lattices, the value space, the service area, a
//! brick and the ways a column makes), and asks the content it draws for that content's rules
//! (`content`). pxl-render's WGSL passes are written by hand to it, their layouts are
//! generated from it (pxl-render build.rs), and a test holds each pass word for word to it
//! (pxl-render tests/).
//!
//! Nothing here can fail a check at run time: every index stays inside its array, no divisor
//! is zero, and no range's bounds cross.

pub mod answer;
pub mod beam;
pub mod cast;
pub mod clipmap;
pub mod content;
pub mod count;
pub mod eye;
pub mod face;
pub mod fill;
pub mod ground;
pub mod light;
pub mod node_cache;
pub mod object;
pub mod param;
pub mod raster;
pub mod ray;
pub mod record;
pub mod seed;
pub mod support;
pub mod way;
pub mod world;

use pxl_spec::space::coord::zoom::FINEST_ZOOM;

/// The most workgroups a dispatch takes on an axis: WebGPU's
/// `maxComputeWorkgroupsPerDimension` at its default. The page's device keeps the default,
/// because it asks for no limits, and a native device's adapter allows at least as many.
/// Every count a pass writes for an indirect dispatch stays under it, because the room the
/// pass lists into is no larger.
pub const WORKGROUPS_CAP: u32 = 65_535;

/// A storage buffer's view is bound at a multiple of this many words: WebGPU's
/// `minStorageBufferOffsetAlignment` at its default, 256 bytes, the most any device asks.
pub const STORAGE_ALIGN_WORDS: u32 = 256 / 4;

/// Element `i` of `s`, a buffer the host wrote or a table, read so that no read aborts:
/// outside the slice it is `T::default()`, which means nothing held, void or zero. This is
/// the one rule for what lies outside a buffer. A host that indexes past what it wrote breaks
/// a contract, which the CPU's debug builds catch.
pub fn at<T: Copy + Default>(s: &[T], i: u32) -> T {
    // The check runs only in debug builds.
    #[cfg(debug_assertions)]
    assert!(
        (i as usize) < s.len(),
        "read past a buffer: {i} of {}",
        s.len()
    );
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked(i as usize) }
    } else {
        T::default()
    }
}

/// Component `a` of an axis triple, chosen by comparison rather than indexed, so no read
/// aborts. Past the third component, it is the third.
pub fn axis<T: Copy>(v: [T; 3], a: u32) -> T {
    if a == 0 {
        v[0]
    } else if a == 1 {
        v[1]
    } else {
        v[2]
    }
}

/// The bits a zoom takes wherever it is packed into a word; every zoom fits.
pub const ZOOM_BITS: u32 = 6;
const _: () = assert!(FINEST_ZOOM < 1 << ZOOM_BITS);
