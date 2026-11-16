// The triangle every table's pass draws over its whole target. Its texel
// centers fall at half steps.

struct TableOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> TableOut {
  let p = screen_triangle_corner(i);
  var o: TableOut;
  o.clip = vec4f(p, 0.0, 1.0);
  return o;
}
