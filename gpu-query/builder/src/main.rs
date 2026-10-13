//! Builds the shader crate for `spirv-unknown-naga-wgsl` and prints where
//! the WGSL lands, or the compiler's own error.

use spirv_builder::{Capability, SpirvBuilder};

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/../shader");
    // The caves' hash is u64 over i64 cells, their turn an f64 share, a
    // zoom a u8 (wall-int64.txt, wall-int8.txt): asked for, so the next wall
    // is Naga's or WGSL's.
    let result = SpirvBuilder::new(shader, "spirv-unknown-naga-wgsl")
        .capability(Capability::Int64)
        .capability(Capability::Float64)
        .build();
    match result {
        Ok(r) => println!("built: {:?}", r.module),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
