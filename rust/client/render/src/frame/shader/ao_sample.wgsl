// Samples the occlusion at half resolution over the hemisphere about each
// texel's normal. It writes the occlusion, 1 where nothing occludes, and the
// normal packed into the other three channels for the denoise and the apply.
// The normal is found here from the full-resolution depth at the pixel the
// downsample kept, so the half-resolution depth that every tap reads holds
// one float a texel.

@group(0) @binding(1) var half_depth: texture_2d<f32>;
@group(0) @binding(2) var blue_noise: texture_2d<f32>;
@group(0) @binding(3) var scene_depth: texture_depth_2d;

// Sample direction `k` of the hemisphere's `AO_SAMPLES`, as N8AO's
// `generateHemisphereSamples(4)` gives them, picked by selects: a phone's driver
// refuses a table of vectors read at a place found as the shader runs.
fn hemisphere(k: i32) -> vec3f {
  let odd = (k & 1) != 0;
  let low = select(vec3f(0.353553, 0.000000, 0.935414), vec3f(-0.451544, 0.413652, 0.790569), odd);
  let high = select(vec3f(0.069116, -0.787542, 0.612372), vec3f(0.569142, 0.742346, 0.353553), odd);
  return select(low, high, k >= 2);
}

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(half_depth, clamp(p, vec2i(0), vec2i(ao_half()) - 1), 0).r;
}

fn scene_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(frame_viewport()) - 1), 0);
}

// The view-space normal at pixel `p` of depth `c0`. On each axis it takes the
// slope toward the side whose second difference is smaller, so a pixel at an
// edge takes the slope of its own surface (N8AO's `computeNormal`).
fn view_normal(p: vec2i, c0: f32) -> vec3f {
  let res = frame_viewport();
  let uv = (vec2f(p) + 0.5) / res;
  let l2 = scene_at(p - vec2i(2, 0));
  let l1 = scene_at(p - vec2i(1, 0));
  let r1 = scene_at(p + vec2i(1, 0));
  let r2 = scene_at(p + vec2i(2, 0));
  let b2 = scene_at(p - vec2i(0, 2));
  let b1 = scene_at(p - vec2i(0, 1));
  let t1 = scene_at(p + vec2i(0, 1));
  let t2 = scene_at(p + vec2i(0, 2));
  let dl = abs((2.0 * l1 - l2) - c0);
  let dr = abs((2.0 * r1 - r2) - c0);
  let db = abs((2.0 * b1 - b2) - c0);
  let dt = abs((2.0 * t1 - t2) - c0);
  let ce = view_pos(c0, uv);
  let px = vec2f(1.0 / res.x, 0.0);
  let py = vec2f(0.0, 1.0 / res.y);
  let dpdx = select(view_pos(r1, uv + px) - ce, ce - view_pos(l1, uv - px), dl < dr);
  let dpdy = select(view_pos(t1, uv + py) - ce, ce - view_pos(b1, uv - py), db < dt);
  return normalize(cross(dpdy, dpdx));
}

// The pixel, of the 2×2 under half texel `hp`, whose depth the downsample
// kept: the first in order whose depth is `depth`. When none of the first
// three is, it is the fourth.
fn full_px(hp: vec2i, depth: f32) -> vec2i {
  let fp = hp * i32(AO_DOWNSCALE);
  for (var k = 0; k < 3; k++) {
    let at = fp + vec2i(k & 1, k >> 1);
    if (scene_at(at) == depth) {
      return at;
    }
  }
  return fp + vec2i(1, 1);
}

fn rot_z(theta: f32) -> mat3x3f {
  let c = cos(theta);
  let s = sin(theta);
  return mat3x3f(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let hp = vec2i(i.clip.xy);
  let depth = textureLoad(half_depth, hp, 0).r;
  if (depth == 0.0) {
    return vec4f(1.0);
  }
  let uv = i.clip.xy / ao_half();
  let pos = view_pos(depth, uv);
  let normal = view_normal(full_px(hp, depth), depth);
  let noise = textureLoad(blue_noise, hp & vec2i(AO_NOISE_MASK), 0);
  var helper = vec3f(0.0, 1.0, 0.0);
  if (dot(helper, normal) > 0.99) {
    helper = vec3f(1.0, 0.0, 0.0);
  }
  let tangent = normalize(cross(helper, normal));
  let bitangent = cross(normal, tangent);
  let tbn = mat3x3f(tangent, bitangent, normal) * rot_z(noise.r * TAU);
  let falloff = AO_RADIUS * AO_DIST_FALLOFF * AO_FALLOFF_SHARE;
  var occluded = 0.0;
  var total = 0.0;
  var off_move = noise.g;
  let off_step = 1.0 / f32(AO_SAMPLES);
  for (var k = 0; k < AO_SAMPLES; k++) {
    let dir = tbn * hemisphere(k);
    let move_amount = fract(off_move);
    off_move += off_step;
    let sample_pos = pos + AO_RADIUS * move_amount * dir;
    let o = ao_proj() * vec4f(sample_pos, 1.0);
    let ndc = o.xyz / o.w;
    let s = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (all(s * (1.0 - s) > vec2f(0.0)) && ndc.z > 0.0 && ndc.z < 1.0) {
      let texel = floor(s * ao_half());
      let sample_depth = textureLoad(half_depth, vec2i(texel), 0).r;
      if (sample_depth > 0.0) {
        let dist_sample = view_dist(sample_depth);
        let dist_world = view_dist(ndc.z);
        let range_check = smoothstep(0.0, 1.0, falloff / abs(dist_sample - dist_world));
        // `i.clip.xy` lies half a texel past the corner `texel` names, so a
        // squared distance under one means the sample fell in this texel
        // itself, which does not occlude it.
        let diff = i.clip.xy - texel;
        occluded += range_check * f32(dist_sample != dist_world) * f32(sample_depth != depth)
          * step(dist_sample, dist_world) * step(1.0, dot(diff, diff));
      }
      total += 1.0;
    }
  }
  let occ = clamp(1.0 - occluded / select(total, 1.0, total == 0.0), 0.0, 1.0);
  return vec4f(occ, 0.5 + 0.5 * normal);
}
