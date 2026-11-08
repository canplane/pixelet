// pxl-render/frame/shader/light_mask.wgsl

// The gate the light's map is drawn through where only the listed
// casters that changed moved over the world its cast left as it was (gpu/shadow.rs
// `begin`), as the world's pass is through its own (screen_mask.wgsl): in
// the map's claimed tiles, a quad a tile of the marks' box, the unclaimed
// collapsed (screen_tiles.wgsl `rect_clip`), the world's depth laid
// (light_lay.wgsl) and the stencil raised, which the casters' depth tests.

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  return rect_clip(vi, ii);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
