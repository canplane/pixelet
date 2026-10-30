// pxl-render/frame/shader/screen_resolve.wgsl

// The world's color gathered over frames (gpu/resolve.rs): this frame's,
// each pixel's ray off its middle by the frame's jitter (frame.wgsl
// `jittered`), blended into what the frames before left where this pixel's
// middle was in the last, found by the nearest depth about it moved into the
// last frame's view (`reproject`). What the frames before left is first held
// within what this frame's pixels about it hold, in luma and chroma, so what
// moved, came or went leaves no trail behind it.

struct Resolve {
  // A pixel's middle and depth in this frame's view, unjittered, to its
  // place in the last frame's.
  reproject: mat4x4f,
  // The share this frame's color takes where its tile changed this frame
  // and the last frame's view held the place, 1 where nothing before is
  // kept.
  blend: vec4f,
};

@group(0) @binding(1) var world: texture_2d<f32>;
@group(0) @binding(2) var world_depth: texture_depth_2d;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var history_sampler: sampler;
@group(0) @binding(5) var<uniform> resolve: Resolve;
// Each tile's age (screen_tiles.wgsl `tile_age`): 0 where it changed this frame,
// what the frames before left held to this frame's pixels; past it, only the
// jitter moved there since, and the frame takes its even share of the
// sequence, what they left standing whole.

// How far, in the spread of the pixels about it, what the frames before
// left may lie from their mean.
const RESOLVE_SPREAD: f32 = 1.0;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

// The claimed tiles alone (screen_tiles.wgsl).
@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}

fn resolve_ycocg(c: vec4f) -> vec4f {
  return vec4f(
    0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
    0.5 * c.r - 0.5 * c.b,
    -0.25 * c.r + 0.5 * c.g - 0.25 * c.b,
    c.a,
  );
}

fn resolve_rgb(y: vec4f) -> vec4f {
  return vec4f(y.x + y.y - y.z, y.x + y.z, y.x - y.y - y.z, y.w);
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let size = frame_viewport();
  let last = vec2i(size) - 1;
  let p = vec2i(v.clip.xy);
  let c = resolve_ycocg(textureLoad(world, p, 0));
  // The pixel and the four beside it: their mean and spread, and the
  // nearest depth, the greatest reversed, so an edge's pixel follows what
  // stands before it.
  var m1 = c;
  var m2 = c * c;
  var depth = textureLoad(world_depth, p, 0);
  let beside = array<vec2i, 4>(vec2i(-1, 0), vec2i(1, 0), vec2i(0, -1), vec2i(0, 1));
  for (var k = 0; k < 4; k++) {
    let q = clamp(p + beside[k], vec2i(0), last);
    let s = resolve_ycocg(textureLoad(world, q, 0));
    m1 += s;
    m2 += s * s;
    depth = max(depth, textureLoad(world_depth, q, 0));
  }
  m1 /= 5.0;
  let spread = sqrt(max(m2 / 5.0 - m1 * m1, vec4f(0.0))) * RESOLVE_SPREAD;
  // Where its middle was in the last frame's view.
  let ndc = vec2f(v.clip.x / size.x * 2.0 - 1.0, 1.0 - v.clip.y / size.y * 2.0);
  let was = resolve.reproject * vec4f(ndc, depth, 1.0);
  let at = was.xy / was.w;
  let uv = vec2f(at.x * 0.5 + 0.5, 0.5 - at.y * 0.5);
  let tile = vec2u(p) / tile_side();
  let age = tile_age(tile.y * tiles.count.x + tile.x);
  let still = age > 0u;
  var blend = select(resolve.blend.x, 1.0 / f32(min(age - 1u, TILE_JITTERS - 2u) + 2u), still);
  if (!(was.w > 0.0) || any(uv < vec2f(0.0)) || any(uv > vec2f(1.0))) {
    blend = 1.0;
  }
  // What the frames before left, held toward the mean within the spread.
  let h = resolve_ycocg(textureSampleLevel(history, history_sampler, uv, 0.0));
  let off = h - m1;
  let room = max(spread, vec4f(1e-5));
  let k = max(max(abs(off.x) / room.x, abs(off.y) / room.y), max(abs(off.z) / room.z, abs(off.w) / room.w));
  let held = select(h, m1 + off / k, k > 1.0 && !still);
  return resolve_rgb(mix(held, c, blend));
}
