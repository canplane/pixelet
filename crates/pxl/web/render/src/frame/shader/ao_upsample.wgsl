// The occlusion at the frame's resolution, one byte a pixel, for the lit
// passes to read: the 3x3 of half-resolution pixels around each, weighed
// by distance off the pixel's own tangent plane and by normal, the
// full-resolution depth giving the plane, so a half-resolution edge lands
// on the full-resolution one; the plain bilinear read where no tap
// agrees. Then N8AO's intensity as a power, and the fog's lift.

@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(0) @binding(2) var half_depth: texture_2d<f32>;
@group(0) @binding(3) var blurred: texture_2d<f32>;
@group(0) @binding(4) var blurred_sampler: sampler;

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(ao_full()) - 1), 0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let fp = vec2i(i.clip.xy);
  let depth = depth_at(fp);
  if (depth == 0.0) {
    return vec4f(1.0);
  }
  let uv = i.clip.xy / ao_full();
  let pos = view_pos(depth, uv);
  let normal = view_normal(fp, ao_full(), uv);
  let half_max = vec2i(ao_half()) - 1;
  var total = 0.0;
  var acc = vec4f(0.0);
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let p = clamp(fp / 2 + vec2i(x, y), vec2i(0), half_max);
      // The tap's corner, as the compositer placed it.
      let p_uv = vec2f(p) / ao_half();
      let tap_depth = textureLoad(half_depth, p, 0).r;
      let tap = textureLoad(blurred, p, 0);
      let tap_normal = tap.gba * 2.0 - 1.0;
      let tap_pos = view_pos(tap_depth, p_uv);
      let plane_dist = abs(dot(tap_pos - pos, normal));
      let weight = exp(-plane_dist / AO_DISTANCE_FALLOFF) * max(dot(normal, tap_normal), 0.0);
      total += weight;
      acc += tap * weight;
    }
  }
  var texel: vec4f;
  if (total > 0.0) {
    texel = acc / total;
  } else {
    texel = textureSampleLevel(blurred, blurred_sampler, uv, 0.0);
  }
  var occlusion = pow(texel.r, AO_INTENSITY);
  let fog = smoothstep(ao_fog_near(), ao_fog_far(), -pos.z);
  occlusion = mix(occlusion, 1.0, fog);
  return vec4f(occlusion, 0.0, 0.0, 1.0);
}
