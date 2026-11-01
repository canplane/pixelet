// The world's depth from the light laid into the light's shadow map where
// the map is drawn whole, before the decor and the actors are drawn into it
// (gpu/shadow.rs): one triangle over the whole map, its fragment the depth
// the world's cast left (light_lay.wgsl), so a stencil could not reject it
// early. Where the map is drawn through its gate, the gate lays it
// (light_gate.wgsl).

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  return vec4f(p[vi], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  return light_lay(clip);
}
