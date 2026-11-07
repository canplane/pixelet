// The world's depth from the light laid into the light's shadow map in the tiles a receiver
// reads, before the decor and the characters are drawn into it (gpu/shadow.rs): a quad a tile
// listed read (pxl-engine `light::LISTED_READ`), an instance each, as many as the light's
// cast opened (`light::ARGS_LAY`), its fragment the depth the world's cast left
// (light_lay.wgsl); the rest of the map cleared to the far plane, which no receiver reads.
// Where the map is drawn through its gate, the gate lays it (light_gate.wgsl).

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  let w = reads[lay.read.x + 1u + ii];
  let tile = vec2f(f32(w & 0xffffu), f32(w >> 16u));
  // The quad's two triangles, each corner's step across and down worked out, not read off a
  // table.
  let k = vi % LAY_VERTICES;
  let step = vec2f(f32(k == 1u || k == 2u || k == 4u), f32(k == 2u || k == 4u || k == 5u));
  let side = vec2f(textureDimensions(world_depth));
  let p = (tile + step) * f32(LAY_TILE) / side;
  return vec4f(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
