// The world's depth from the light laid into the light's shadow map where
// the map is drawn whole, before the decor and the actors are drawn into it
// (gpu/shadow.rs): a quad a mark, one of every tile (screen_tiles.wgsl
// `mark_clip`), its fragment the depth the world's cast left
// (light_lay.wgsl), so a stencil could not reject it early. Where the map is
// drawn through its gate, the gate lays it (light_gate.wgsl).

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  return mark_clip(vi, ii);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
