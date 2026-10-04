// pxl-render/frame/shader/badge.wgsl

// Billboards drawn after the world. Whether the labeled point is visible is
// decided per badge in the vertex shader; what stands in front of the badge
// is the depth test's, against an upright twin of the quad.

// Hidden when three of five samples up the badge are behind the world.
const DEPTH_BIAS_U: f32 = 1.0;
const DEPTH_BIAS_CELLS: f32 = 1.5;
const DEPTH_SAMPLES: u32 = 5u;
const DEPTH_HIDE_AT: u32 = 3u;
const DEPTH_LIFT_U: f32 = 2.0;

struct Badge {
  view: mat4x4f,
  proj: mat4x4f,
  translation: vec3f,
  px_k: f32,
  fade_start: f32,
  fade_end: f32,
  near: f32,
  far: f32,
  cell: f32,
  _p: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> badge_words: array<vec4f, 10>;

fn badge_view() -> mat4x4f { return mat4x4f(badge_words[0], badge_words[1], badge_words[2], badge_words[3]); }
fn badge_proj() -> mat4x4f { return mat4x4f(badge_words[4], badge_words[5], badge_words[6], badge_words[7]); }
fn badge_translation() -> vec3f { return badge_words[8].xyz; }
fn badge_px_k() -> f32 { return badge_words[8].w; }
fn badge_near() -> f32 { return badge_words[9].x; }
fn badge_far() -> f32 { return badge_words[9].y; }
fn badge_cell() -> f32 { return badge_words[9].z; }

@group(1) @binding(1) var<storage, read> instances: array<f32>;
@group(2) @binding(0) var sheet: texture_2d<f32>;
@group(2) @binding(1) var sheet_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) opacity: f32,
  @location(2) upright_z: f32,
  @location(3) @interpolate(flat) hidden: u32,
};

// Reversed depth through GL's clip z, so the formula is three's.
fn view_distance(d: f32) -> f32 {
  let z = depth_to_gl_z(d);
  return (2.0 * badge_near() * badge_far()) / (badge_far() + badge_near() - z * (badge_far() - badge_near()));
}

// A point off the screen has no depth to read and counts as visible.
fn behind_world(clip: vec4f) -> bool {
  if (clip.w <= 0.0) {
    return false;
  }
  let ndc = clip.xyz / clip.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv >= vec2f(1.0))) {
    return false;
  }
  let dims = vec2f(textureDimensions(world_depth));
  let world = view_distance(textureLoad(world_depth, vec2i(uv * dims), 0));
  let point = view_distance(ndc.z);
  return world < point - DEPTH_BIAS_U - point * badge_cell() * DEPTH_BIAS_CELLS;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  // BILLBOARD_FLOATS floats an instance.
  let k = ii * 10u;
  let anchor = on_globe(vec3f(instances[k], instances[k + 1u], instances[k + 2u]) + badge_translation());
  let size_px = vec2f(instances[k + 3u], instances[k + 4u]);
  let rect = vec4f(instances[k + 5u], instances[k + 6u], instances[k + 7u], instances[k + 8u]);
  let frac = instances[k + 9u];
  let corners = array<vec2f, 4>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5));
  let q = corners[vi];

  let anchor_view = (badge_view() * vec4f(anchor, 1.0)).xyz;
  let dist = length(anchor_view);
  let depth = max(-anchor_view.z, 1e-3);
  let fade = 1.0 - smoothstep(badge_fade_start(), badge_fade_end(), dist);
  let size = size_px * depth / badge_px_k() * fade;
  let drop = size.y * (frac - 0.5);
  let center_view = anchor_view + vec3f(0.0, drop, 0.0);
  let corner = q * size;

  var o: VertexOut;
  o.clip = badge_proj() * vec4f(center_view + vec3f(corner, 0.0), 1.0);
  o.uv = vec2f(mix(rect.x, rect.z, q.x + 0.5), mix(rect.w, rect.y, q.y + 0.5));
  o.opacity = fade;

  let view_proj = badge_proj() * badge_view();
  let lifted = view_proj * vec4f(anchor + vec3f(0.0, 0.0, DEPTH_LIFT_U), 1.0);
  let rise = view_proj * vec4f(0.0, 0.0, size.y, 0.0);
  var hidden = 0u;
  for (var s = 0u; s < DEPTH_SAMPLES; s++) {
    if (behind_world(lifted + rise * (f32(s) / f32(DEPTH_SAMPLES - 1u)))) {
      hidden++;
    }
  }
  o.hidden = select(0u, 1u, hidden >= DEPTH_HIDE_AT);

  let world_up_view = normalize((badge_view() * vec4f(0.0, 0.0, 1.0, 0.0)).xyz);
  let push = DEPTH_BIAS_U + max(0.0, -center_view.z) * badge_cell() * DEPTH_BIAS_CELLS;
  let upright = badge_proj() * vec4f(center_view + vec3f(corner.x, 0.0, push) + world_up_view * corner.y, 1.0);
  o.upright_z = clamp(upright.z / upright.w, 0.0, 1.0);
  return o;
}

struct FragOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(i: VertexOut) -> FragOut {
  let texel = textureSample(sheet, sheet_sampler, i.uv);
  if (i.hidden != 0u) {
    discard;
  }
  var o: FragOut;
  // The sheet is premultiplied so its mips filter without a dark fringe.
  let color = texel.rgb / max(texel.a, 1e-4);
  o.color = vec4f(to_srgb(grade(color) * vignette_of(i.clip.xy, frame_viewport(), frame_vignette())), texel.a);
  o.depth = i.upright_z;
  return o;
}
