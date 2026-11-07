// pxl-render/frame/shader/lit_skin.wgsl

// A lit model's fragment over its skin, decor's and a character's.

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return lit_color(textureSample(skin, skin_sampler, i.uv).rgb, i);
}
