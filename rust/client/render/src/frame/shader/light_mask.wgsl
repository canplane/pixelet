// pxl-render/frame/shader/light_mask.wgsl

// The gate the light's map is drawn through when the only change is listed
// casters that moved, and the world's cast left the world as it was
// (pass/shadow.rs `begin`). The world's pass is drawn through a gate of its own
// alike (screen_mask.wgsl). The gate draws a quad for each tile of the marks'
// box and collapses the tiles not damaged (screen_tile.wgsl `rect_clip`). In
// the damaged tiles it lays the world's depth (light_lay.wgsl) and raises the
// stencil, which the casters' draws test.

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  return rect_clip(vi, ii);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
