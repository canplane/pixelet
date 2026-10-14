//! Builds the shader crate for `spirv-unknown-naga-wgsl` and prints where
//! the WGSL lands, or the compiler's own error: each entry a module of its
//! own, as a pipeline takes its module's one entry; or with `--harness` the
//! harness's entries too, in one module, into a target of their own.

use spirv_builder::SpirvBuilder;

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/../shader");
    // No capability asked for: WebGPU's own types alone.
    let builder = SpirvBuilder::new(shader, "spirv-unknown-naga-wgsl");
    let builder = match std::env::args().any(|a| a == "--harness") {
        true => builder.shader_crate_features(["harness".to_string()]).target_dir_path("spirv-builder-harness"),
        false => builder.multimodule(true),
    };
    let result = builder.build();
    match result {
        Ok(r) => println!("built: {:?}", r.module),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
