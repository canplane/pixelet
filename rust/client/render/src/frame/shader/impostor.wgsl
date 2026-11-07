// Decor instances too small on screen for their model, drawn as quads placed
// as impostor_place.wgsl lays them.

@group(2) @binding(0) var impostors: texture_2d_array<f32>;
@group(2) @binding(1) var impostor_sampler: sampler;

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let p = placed(v);
  return lit_vertex(p.world, vec2f(0.5 + 0.5 * p.corner.x, 0.5 - 0.5 * p.corner.y), p.facing, instance_opacity());
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let texel = textureSample(impostors, impostor_sampler, i.uv, instance_layer());
  if (texel.a < 0.5) {
    discard;
  }
  return lit_color(texel.rgb, i);
}
