// pxl-render/frame/shader/marker.wgsl

// Markers are drawn after the world as billboards, in two passes over the same
// instances. Each marker stands in the world as an upright twin of its quad,
// which gives the marker its depth. A marker keeps a region about itself, clear
// and centered on what it draws, and asks at one point, its anchor, whether it
// is seen. marker_region.wgsl draws every region whose anchor the world leaves
// seen, each as its marker's number, and the nearest region wins a pixel.
// marker_sprite.wgsl draws a marker only where the number at its anchor is its
// own. So a marker is seen whole or not at all, and two markers seen may still
// overlap.

// A twin stands this far toward the eye, a unit and one and a half camera grid
// cells at its depth, so a marker is not lost in the surface it stands on.
const DEPTH_BIAS_U: f32 = 1.0;
const DEPTH_BIAS_CELLS: f32 = 1.5;

// The uniform is read field by field from vec4 words and is never declared as a
// struct. Declared as a struct here, it makes Safari's WebGPU lose its GPU
// process. It holds the view, the projection, `px_per_unit`, and the number
// before the set's first marker.
@group(1) @binding(0) var<uniform> marker_words: array<vec4f, MARKER_VEC4S>;

// Each marker's words: the Q3 position it stands at, then its floats (word.rs
// `MARKER_INSTANCE_WORDS`).
@group(2) @binding(0) var<storage, read> instances: array<u32>;

fn instance_f32(k: u32, w: u32) -> f32 {
  return bitcast<f32>(instances[k + w]);
}

// A marker's number. It is never 0, which stands for no region.
fn marker_number(ii: u32) -> u32 {
  return u32(marker_before()) + ii + 1u;
}

struct Placement {
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

// Corner `vi` of the quad of instance `ii`, as wide as its region where
// `region`, else as wide as its sprite.
fn placed(vi: u32, ii: u32, region: bool) -> Placement {
  let k = ii * MARKER_INSTANCE_WORDS;
  // The marker stands on Q3, as the draws do (gpu/draw.rs `Position`), and is
  // taken against the eye's Q3 the short way round X.
  let stands = vec3i(bitcast<i32>(instances[k]), bitcast<i32>(instances[k + 1u]), bitcast<i32>(instances[k + 2u]));
  let anchor = from_eye(stands, vec3f(0.0));
  let sprite_w = instance_f32(k, 3u);
  let size_px = vec2f(select(sprite_w, instance_f32(k, 10u), region), instance_f32(k, 4u));
  let frac = instance_f32(k, 9u);
  let corners = array<vec2f, 4>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5));
  let q = corners[vi];

  let anchor_view = (marker_view() * vec4f(anchor, 1.0)).xyz;
  let depth = max(-anchor_view.z, 1e-3);
  let size = size_px * depth / marker_px_per_unit();
  let drop = size.y * (frac - 0.5);
  let center_view = anchor_view + vec3f(0.0, drop, 0.0);
  // A region reaches a pixel past its sprite above and below, so an anchor on
  // the sprite's edge, a bubble's tail tip, is inside its own region whatever
  // the subpixel it falls on.
  let corner = q * (size + vec2f(0.0, select(0.0, 2.0, region) * depth / marker_px_per_unit()));

  let clip = marker_proj() * vec4f(center_view + vec3f(corner, 0.0), 1.0);
  let across = q.x * size_px.x / sprite_w + 0.5;

  let world_up_view = normalize((marker_view() * vec4f(0.0, 0.0, 1.0, 0.0)).xyz);
  let push = DEPTH_BIAS_U + max(0.0, -center_view.z) * GRID_CELL * DEPTH_BIAS_CELLS;
  // The part of the quad that hangs under the anchor lies on the level ground
  // through the anchor, where the eye sees it.
  let over = drop + corner.y;
  var twin = anchor_view + vec3f(corner.x, 0.0, 0.0) + world_up_view * max(over, 0.0);
  let shown = center_view + vec3f(corner, 0.0);
  let toward = dot(shown, world_up_view);
  let t = dot(anchor_view, world_up_view) / toward;
  if (over < 0.0 && abs(toward) > 1e-6 && t > 0.0 && t < 1.0) {
    twin = shown * t;
  }
  let upright = marker_proj() * vec4f(twin + vec3f(0.0, 0.0, push), 1.0);
  return Placement(clip, clamp(upright.z / upright.w, 0.0, 1.0), across, q, anchor_view, push);
}

// The pixel of a target `dims` large that the anchor falls in. Its x is below 0
// where the anchor is off the screen.
fn anchor_px(p: Placement, dims: vec2u) -> vec2i {
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

// A clip position that every vertex of a quad given it shares, so the quad has no area and
// nothing of it is drawn.
const NOWHERE: vec4f = vec4f(0.0, 0.0, 0.0, 1.0);
