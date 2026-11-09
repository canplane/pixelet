// The lit pass over the characters, which places each vertex as skinned_place.wgsl lays it.

@vertex
fn vs(v: VertexIn) -> VertexOut {
  draw_at(v.instance);
  let m = skin_of(v);
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  return lit_vertex(skinned_world(v, m), v.uv, place_up(n, skinned_rot()), 1.0);
}
