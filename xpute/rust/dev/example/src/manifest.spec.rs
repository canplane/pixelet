// rust/dev/example/src/manifest.spec.rs
//
// GENERATED from rust/dev/example/manifest.json

//! The manifest its JSON declares (xpute-core manifest.rs), which a build
//! script includes too as it links the module.

use xpute_core::manifest::Manifest;

pub const MANIFEST: Manifest = Manifest {
    mem_bytes: &[2097152],
    region_bytes: 2097152,
    min_heap_bytes: 65536,
    stack_bytes: 65536,
    thread_stack_bytes: 1048576,
    pkt_bytes: 256,
    fs: false,
    stream: None,
};

// The runtime's MANIFEST places the front's parts at these same numbers.
const _: () = assert!(MANIFEST.head_at() == 65536);
const _: () = assert!(MANIFEST.stream_at() == 0);
