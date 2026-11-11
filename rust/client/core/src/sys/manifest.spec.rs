// rust/client/core/src/sys/manifest.spec.rs
//
// GENERATED from spec/client/core/sys/manifest.json

//! The manifest its JSON declares (xpute-core manifest.rs), which a build
//! script includes too as it links the module.

use xpute_core::manifest::{Manifest, Stream};

pub const MANIFEST: Manifest = Manifest {
    mem_bytes: &[1073741824, 536870912],
    region_bytes: 67108864,
    min_heap_bytes: 268435456,
    stack_bytes: 1048576,
    thread_stack_bytes: 8388608,
    pkt_bytes: 4096,
    fs: true,
    stream: Some(Stream { bytes: 4194304 }),
};

// The runtime's MANIFEST places the front's parts at these same numbers.
const _: () = assert!(MANIFEST.head_at() == 1048576);
const _: () = assert!(MANIFEST.stream_at() == 1052672);

// What the core asks of its GPU: the power preference, and the features
// the device must have and those it takes where the adapter has them.
pub const GPU_ASK: (&str, &[&str], &[&str]) = (
    "high-performance",
    &["depth32float-stencil8"],
    &["timestamp-query"],
);
