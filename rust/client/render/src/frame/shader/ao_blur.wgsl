// Half-resolution AO denoise: a blue-noise-spun Poisson disc whose taps are
// weighed by plane distance and normal, so the blur never crosses an edge.

@group(0) @binding(1) var half_depth: texture_2d<f32>;
@group(0) @binding(2) var blue_noise: texture_2d<f32>;
@group(0) @binding(3) var src: texture_2d<f32>;
@group(0) @binding(4) var src_sampler: sampler;

struct Run {
  index: f32,
};

@group(0) @binding(5) var<uniform> run: Run;

// N8AO's generateDenoiseSamples(4, 11).
const DISC = array<vec2f, 4>(
  vec2f(0.353553, 0.000000),
  vec2f(0.000000, -0.594604),
  vec2f(-0.805927, 0.000000),
  vec2f(0.000000, 1.000000),
);

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(half_depth, clamp(p, vec2i(0), vec2i(ao_half()) - 1), 0).r;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let hp = vec2i(i.clip.xy);
  let data = textureLoad(src, hp, 0);
  let depth = depth_at(hp);
  if (depth == 0.0) {
    return data;
  }
  let uv = i.clip.xy / ao_half();
  let texel = 1.0 / ao_half();
  let normal = data.gba * 2.0 - 1.0;
  let pos = view_pos(depth, uv);
  let noise = textureLoad(blue_noise, hp & vec2i(AO_NOISE_MASK), 0);
  let angle = select(noise.z, noise.w, run.index < 0.5) * TAU;
  let c = cos(angle);
  let s = sin(angle);
  let spin = mat2x2f(c, -s, s, c);
  let falloff = AO_RADIUS * AO_DISTANCE_FALLOFF * AO_FALLOFF_SHARE;
  var occlusion = data.r;
  var count = 1.0;
  var disc = DISC;
  for (var k = 0; k < AO_DENOISE_SAMPLES; k++) {
    let at = uv + (spin * disc[k]) * texel * AO_DENOISE_RADIUS;
    let tap = textureSampleLevel(src, src_sampler, at, 0.0);
    let tap_normal = tap.gba * 2.0 - 1.0;
    let tap_depth = depth_at(vec2i(at * ao_half()));
    let tap_pos = view_pos(tap_depth, at);
    let plane_dist = abs(dot(tap_pos - pos, normal));
    let weight = f32(tap_depth != 0.0) * exp(-plane_dist / falloff) * max(dot(normal, tap_normal), 0.0);
    occlusion += tap.r * weight;
    count += weight;
  }
  occlusion = clamp(occlusion / count, 0.0, 1.0);
  if (occlusion == 0.0) {
    occlusion = 1.0;
  }
  return vec4f(occlusion, 0.5 + 0.5 * normal);
}
