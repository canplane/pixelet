// The billboards — the POI badges and the chat bubble: an instanced quad
// over the module's billboard sets (billboard.rs), one draw a set, drawn
// after the world with the world's depth attached read-only and bound as
// a texture at once, so both of anchor_occlusion.ts's questions
// (archive/20260914-three-draw-paths) are asked of this frame's depth.
// Whether the labelled point is visible is asked in the vertex shader, at
// five points up the badge from just above the anchor, and answered for
// the whole badge; whether something stands between the camera and the
// badge itself is the depth test's, against the depth of an upright twin
// of the quad written as the fragment's.

// anchor_occlusion.ts's tolerances and sampling: a unit of slack plus a
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
};

@group(0) @binding(0) var<uniform> badge: Badge;
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
  return (2.0 * badge.near * badge.far) / (badge.far + badge.near - z * (badge.far - badge.near));
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
  return world < point - DEPTH_BIAS_U - point * badge.cell * DEPTH_BIAS_CELLS;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  let k = ii * 10u;
  let anchor = vec3f(instances[k], instances[k + 1u], instances[k + 2u]) + badge.translation;
  let size_px = vec2f(instances[k + 3u], instances[k + 4u]);
  let rect = vec4f(instances[k + 5u], instances[k + 6u], instances[k + 7u], instances[k + 8u]);
  let frac = instances[k + 9u];
  let corners = array<vec2f, 4>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5));
  let q = corners[vi];

  let anchor_view = (badge.view * vec4f(anchor, 1.0)).xyz;
  let dist = length(anchor_view);
  let depth = max(-anchor_view.z, 1e-3);
  let fade = 1.0 - smoothstep(badge.fade_start, badge.fade_end, dist);
  let size = size_px * depth / badge.px_k * fade;
  let drop = size.y * (frac - 0.5);
  let centre_view = (badge.view * vec4f(anchor + vec3f(0.0, 0.0, drop), 1.0)).xyz;
  let corner = q * size;

  var o: VertexOut;
  o.clip = badge.proj * vec4f(centre_view + vec3f(corner, 0.0), 1.0);
  // The sheet's v0 is the rectangle's top edge.
  o.uv = vec2f(mix(rect.x, rect.z, q.x + 0.5), mix(rect.w, rect.y, q.y + 0.5));
  o.opacity = fade;

  let view_proj = badge.proj * badge.view;
  let lifted = view_proj * vec4f(anchor + vec3f(0.0, 0.0, DEPTH_LIFT_U), 1.0);
  let rise = view_proj * vec4f(0.0, 0.0, size.y, 0.0);
  var hidden = 0u;
  for (var s = 0u; s < DEPTH_SAMPLES; s++) {
    if (behind_world(lifted + rise * (f32(s) / f32(DEPTH_SAMPLES - 1u)))) {
      hidden++;
    }
  }
  o.hidden = select(0u, 1u, hidden >= DEPTH_HIDE_AT);

  let world_up_view = normalize((badge.view * vec4f(0.0, 0.0, 1.0, 0.0)).xyz);
  let push = DEPTH_BIAS_U + max(0.0, -centre_view.z) * badge.cell * DEPTH_BIAS_CELLS;
  let upright = badge.proj * vec4f(centre_view + vec3f(corner.x, 0.0, push) + world_up_view * corner.y, 1.0);
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
  // Vignetted as the world under it is: the post stack darkened the whole
  // screen, badges included.
  o.color = vec4f(to_srgb(grade(texel.rgb) * vignette_of(i.clip.xy, badge.viewport, badge.vignette)), texel.a * i.opacity);
  o.depth = i.upright_z;
  return o;
}
