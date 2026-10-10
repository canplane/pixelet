// pxl-render/frame/shader/record.test.rs

use naga::valid::{Capabilities, ValidationFlags, Validator};

use super::{source, SHADERS};

/// What a browser's WebGPU refuses at pipeline creation, failing the frame far
/// from the line, is caught here.
#[test]
fn every_shader_compiles_and_validates() {
    for kind in 0..SHADERS {
        let src = source(kind);
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("shader {kind}: {}", e.emit_to_string(src)));
        if let Err(e) = Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(&module) {
            panic!("shader {kind}: {}", e.emit_to_string(src));
        }
    }
}
