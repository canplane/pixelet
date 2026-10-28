// pxl-render/frame/shader/marker.wgsl

// Markers drawn after the world as billboards, each standing in it as an
// upright twin of its quad, in two passes over the same instances. A marker
// keeps a region about itself, clear and centered on what it draws, and asks
// at one point, its anchor, whether it is seen: marker_region.wgsl draws every
// region whose anchor the world leaves seen, each as its marker's number, the
// nearest winning a pixel; marker_sprite.wgsl draws a marker only where the number
// at its anchor is its own. So one is seen whole or not at all, and two seen
// may still overlap.

// A twin stands this far toward the eye, so a marker is not lost in the
// surface it stands on.
const DEPTH_BIAS_U: f32 = 1.0;
const DEPTH_BIAS_CELLS: f32 = 1.5;

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process. view, proj, translation and px_per_unit, then the
// number before the set's first marker.
@group(1) @binding(0) var<uniform> marker_words: array<vec4f, MARKER_VEC4S>;

@group(2) @binding(0) var<storage, read> instances: array<f32>;

// A marker's number, never 0, which is no region.
fn marker_number(ii: u32) -> u32 {
  return u32(marker_before()) + ii + 1u;
}

struct Placed {
  clip: vec4f,
  // The twin's depth at this corner.
  upright_z: f32,
  // Across the sprite, 0 to 1.
  across: f32,
  q: vec2f,
  // The anchor and how far toward the eye a twin stands.
  anchor_view: vec3f,
  push: f32,
};

// Corner `vi` of instance `ii`'s quad, as wide as its sprite or its region.
fn placed(vi: u32, ii: u32, region: bool) -> Placed {
  let k = ii * MARKER_INSTANCE_FLOATS;
  let anchor = vec3f(instances[k], instances[k + 1u], instances[k + 2u]) + marker_translation();
  let sprite_w = instances[k + 3u];
  let size_px = vec2f(select(sprite_w, instances[k + 10u], region), instances[k + 4u]);
  let frac = instances[k + 9u];
  let corners = array<vec2f, 4>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5));
  let q = corners[vi];

  let anchor_view = (marker_view() * vec4f(anchor, 1.0)).xyz;
  let depth = max(-anchor_view.z, 1e-3);
  let size = size_px * depth / marker_px_per_unit();
  let drop = size.y * (frac - 0.5);
  let center_view = anchor_view + vec3f(0.0, drop, 0.0);
  // A region reaches a pixel past its sprite above and below, so an anchor on
  // the sprite's edge, a bubble's tail tip, is inside its own region whatever
  // the subpixel it falls on; on the edge it flickered as it moved.
  let corner = q * (size + vec2f(0.0, select(0.0, 2.0, region) * depth / marker_px_per_unit()));

  var p: Placed;
  p.clip = marker_proj() * vec4f(center_view + vec3f(corner, 0.0), 1.0);
  p.across = q.x * size_px.x / sprite_w + 0.5;
  p.q = q;
  p.anchor_view = anchor_view;

  let world_up_view = normalize((marker_view() * vec4f(0.0, 0.0, 1.0, 0.0)).xyz);
  p.push = DEPTH_BIAS_U + max(0.0, -center_view.z) * frame_cell() * DEPTH_BIAS_CELLS;
  // What hangs under the anchor lies on the level ground through it, where the
  // eye sees it; stood up, it would be in the ground the eye sees in front.
  let over = drop + corner.y;
  var twin = anchor_view + vec3f(corner.x, 0.0, 0.0) + world_up_view * max(over, 0.0);
  let shown = center_view + vec3f(corner, 0.0);
  let toward = dot(shown, world_up_view);
  let t = dot(anchor_view, world_up_view) / toward;
  if (over < 0.0 && abs(toward) > 1e-6 && t > 0.0 && t < 1.0) {
    twin = shown * t;
  }
  let upright = marker_proj() * vec4f(twin + vec3f(0.0, 0.0, p.push), 1.0);
  p.upright_z = clamp(upright.z / upright.w, 0.0, 1.0);
  return p;
}

// The pixel of a target `dims` large the anchor falls in; x < 0 off the screen.
fn anchor_pixel(p: Placed, dims: vec2u) -> vec2i {
  let at = marker_proj() * vec4f(p.anchor_view, 1.0);
  if (at.w <= 0.0) {
    return vec2i(-1);
  }
  let ndc = at.xy / at.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv >= vec2f(1.0))) {
    return vec2i(-1);
  }
  return vec2i(uv * vec2f(dims));
}

// A quad with no area: nothing of it is drawn.
const NOWHERE: vec4f = vec4f(0.0, 0.0, 0.0, 1.0);
