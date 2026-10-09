// pxl-render/frame/shader/badge_sprite.wgsl

// A billboard's sprite where the region nearest at its anchor is its own
// (badge.wgsl); one whose anchor is off the screen is drawn.

@group(1) @binding(2) var nearest: texture_2d<f32>;
@group(2) @binding(0) var sheet: texture_2d<f32>;
@group(2) @binding(1) var sheet_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) upright_z: f32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  let p = placed(vi, ii, false);
  var o: VertexOut;
  o.clip = p.clip;
  let at = anchor_pixel(p, textureDimensions(nearest));
  if (at.x >= 0 && pack4x8unorm(textureLoad(nearest, at, 0)) != badge_number(ii)) {
    o.clip = NOWHERE;
  }
  let k = ii * 11u;
  let rect = vec4f(instances[k + 5u], instances[k + 6u], instances[k + 7u], instances[k + 8u]);
  o.uv = vec2f(mix(rect.x, rect.z, p.across), mix(rect.w, rect.y, p.q.y + 0.5));
  o.upright_z = p.upright_z;
  return o;
}

struct FragOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(i: VertexOut) -> FragOut {
  let texel = textureSample(sheet, sheet_sampler, i.uv);
  var o: FragOut;
  // The sheet is premultiplied so its mips filter without a dark fringe.
  let color = texel.rgb / max(texel.a, 1e-4);
  o.color = vec4f(to_srgb(color * vignette_of(i.clip.xy, frame_viewport(), frame_vignette())), texel.a);
  o.depth = i.upright_z;
  return o;
}
