// pxl-render/frame/shader/light_gate.wgsl

// The gate the light's map is drawn through where only the listed
// casters that changed moved over the world its cast left as it was (gpu/shadow.rs
// `begin`), as the world's pass is through its own (screen_gate.wgsl): the
// map's claimed tiles, a quad a mark or a tile of one (screen_tiles.wgsl
// `mark_clip`), set back to the far plane, which
// a cleared map starts from, and its stencil raised, which the casters' depth
// and the world's laid in test. Depth alone: no fragment stage.

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  var clip = mark_clip(vi, ii);
  clip.z = 1.0;
  return clip;
}
