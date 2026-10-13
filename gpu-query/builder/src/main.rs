//! Builds the shader crate for `spirv-unknown-naga-wgsl` and prints where
//! the WGSL lands, or the compiler's own error.

use spirv_builder::SpirvBuilder;

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/../shader");
    // No capability asked for: WebGPU's own types alone.
    let result = SpirvBuilder::new(shader, "spirv-unknown-naga-wgsl").build();
    match result {
        Ok(r) => println!("built: {:?}", r.module),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
