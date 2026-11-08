// Draws decor as instanced models of its variants, placed as decor_place.wgsl places them.

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let p = placed(v);
  return lit_vertex(p.world, v.uv, place_up(v.normal, p.spin), instance_opacity());
}
