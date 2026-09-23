// One triangle over a table, for the passes that draw the atmosphere's:
// `uv` is the fragment's place on it, texel centres at the half steps.

struct TableOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> TableOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: TableOut;
  o.clip = vec4f(p, 0.0, 1.0);
  return o;
}
