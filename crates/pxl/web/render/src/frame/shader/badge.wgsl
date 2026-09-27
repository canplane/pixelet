// pxl-render/frame/shader/badge.wgsl

// The billboards — the POI badges and the chat bubble: an instanced quad
// over the module's billboard sets (billboard.rs), one draw a set, drawn
// after the world with the world's depth attached read-only and bound as
// a texture at once, so both occlusion questions are asked of this
// frame's depth.
// Whether the labeled point is visible is asked in the vertex shader, at
// five points up the badge from just above the anchor, and answered for
// the whole badge; whether something stands between the camera and the
// badge itself is the depth test's, against the depth of an upright twin
// of the quad written as the fragment's.

// The tolerances and sampling: a unit of slack plus a
// cell and a half of the tier's cell at the badge's distance, five samples
// up the badge from two units above the anchor, hidden when three are
// behind the world.
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
  vignette: f32,
  viewport: vec2f,
  // The frame's planet (globe.wgsl), so a badge stands where its ground is.
  planet: vec4f,
  globe: vec4f,
};

// Held as vec4 words, each field read from its own by `badge_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(0) @binding(0) var<uniform> badge_words: array<vec4f, 13>;

fn badge_view() -> mat4x4f { return mat4x4f(badge_words[0], badge_words[1], badge_words[2], badge_words[3]); }
fn badge_proj() -> mat4x4f { return mat4x4f(badge_words[4], badge_words[5], badge_words[6], badge_words[7]); }
fn badge_translation() -> vec3f { return badge_words[8].xyz; }
fn badge_px_k() -> f32 { return badge_words[8].w; }
fn badge_fade_start() -> f32 { return badge_words[9].x; }
fn badge_fade_end() -> f32 { return badge_words[9].y; }
fn badge_near() -> f32 { return badge_words[9].z; }
fn badge_far() -> f32 { return badge_words[9].w; }
fn badge_cell() -> f32 { return badge_words[10].x; }
fn badge_vignette() -> f32 { return badge_words[10].y; }
fn badge_viewport() -> vec2f { return badge_words[10].zw; }
fn badge_planet() -> vec4f { return badge_words[11]; }
fn badge_globe() -> vec4f { return badge_words[12]; }

@group(0) @binding(1) var<storage, read> instances: array<f32>;
@group(1) @binding(0) var sheet: texture_2d<f32>;
@group(1) @binding(1) var sheet_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) opacity: f32,
  @location(2) upright_z: f32,
  @location(3) @interpolate(flat) hidden: u32,
};

// Distance from the camera, in world units, for a depth-buffer value —
// reversed, as depth.wgsl has it — through GL's clip z, so the formula is
// three's.
fn view_distance(d: f32) -> f32 {
  let z = depth_to_gl_z(d);
  return (2.0 * badge_near() * badge_far()) / (badge_far() + badge_near() - z * (badge_far() - badge_near()));
}

// Whether the world stands in front of a point of the badge, given in clip
// space. A point off the screen has no depth to read and counts as visible.
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
  // ndc.z is the reversed depth the point would be written at.
  let point = view_distance(ndc.z);
  return world < point - DEPTH_BIAS_U - point * badge_cell() * DEPTH_BIAS_CELLS;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  // BILLBOARD_FLOATS (kernel billboard/field.rs) floats an instance.
  let k = ii * 10u;
  let anchor = on_globe_of(vec3f(instances[k], instances[k + 1u], instances[k + 2u]) + badge_translation(), badge_planet(), badge_globe());
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
  // The sprite anchor is a screen offset, independent of camera tilt.
  let center_view = anchor_view + vec3f(0.0, drop, 0.0);
  let corner = q * size;

  var o: VertexOut;
  o.clip = badge_proj() * vec4f(center_view + vec3f(corner, 0.0), 1.0);
  // The sheet's v0 is the rectangle's top edge.
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
  // The sheet holds its colors premultiplied (gpu/sheet.rs), which its mips
  // filter without a dark fringe; the pass lays a straight color over.
  let color = texel.rgb / max(texel.a, 1e-4);
  // Vignetted as the world under it is: the post stack darkened the whole
  // screen, badges included.
  o.color = vec4f(to_srgb(grade(color) * vignette_of(i.clip.xy, badge_viewport(), badge_vignette())), texel.a * i.opacity);
  o.depth = i.upright_z;
  return o;
}
