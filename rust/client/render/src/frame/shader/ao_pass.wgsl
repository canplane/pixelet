// Every pass of the occlusion draws this one triangle, which covers its whole target.

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = screen_triangle_corner(vi);
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  return o;
}
