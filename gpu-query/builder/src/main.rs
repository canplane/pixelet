// gpu-query-builder/main.rs

//! Builds the shader crate for `spirv-unknown-naga-wgsl` and prints where
//! the WGSL lands, or the compiler's own error: each entry a module of its
//! own, as a pipeline takes its module's one entry.

use spirv_builder::SpirvBuilder;

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/../shader");
    // No capability asked for: WebGPU's own types alone.
    let result = SpirvBuilder::new(shader, "spirv-unknown-naga-wgsl").multimodule(true).build();
    match result {
        Ok(r) => println!("built: {:?}", r.module),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
