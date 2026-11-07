// Characters lit, placed as skinned_place.wgsl lays them.

@vertex
fn vs(v: VertexIn) -> VertexOut {
  draw_at(v.instance);
  let m = skin_of(v);
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  return lit_vertex(skinned_world(v, m), v.uv, place_up(n, skinned_rot()), skinned_opacity());
}
