// Lays the world's depth from the light into the shadow map, in the tiles a receiver reads,
// before the decor and the characters are drawn into it (pass/shadow.rs). Each tile listed as
// read (pxl-engine `light::LIST_READS`) is one instance of a quad, as many as the light's cast
// opened (`light::ARGS_LAY`), and its fragments take the depth the world's cast left
// (light_lay.wgsl). The rest of the map is cleared to the far plane, which no receiver reads.
// Where the map is drawn through its gate, the gate lays the depth instead (light_mask.wgsl).

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  let w = reads[lay.read.x + 1u + ii];
  let tile = vec2f(f32(w & 0xffffu), f32(w >> 16u));
  let step = vec2f(quad_corner(vi % LAY_VERTEX_CNT));
  let side = vec2f(textureDimensions(world_depth));
  let p = (tile + step) * f32(LIGHT_TILE) / side;
  return vec4f(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
