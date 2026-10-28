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
  // The share this frame's color takes where the last frame's view held
  // the place, 1 where nothing before is kept; then 1 where what the
  // frames before left is held to this frame's pixels, 0 where only the
  // jitter moved since and it stands whole.
  blend: vec4f,
};

@group(0) @binding(1) var world: texture_2d<f32>;
@group(0) @binding(2) var world_depth: texture_depth_2d;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var history_sampler: sampler;
@group(0) @binding(5) var<uniform> resolve: Resolve;

// How far, in the spread of the pixels about it, what the frames before
// left may lie from their mean.
const RESOLVE_SPREAD: f32 = 1.0;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
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
  var blend = resolve.blend.x;
  if (!(was.w > 0.0) || any(uv < vec2f(0.0)) || any(uv > vec2f(1.0))) {
    blend = 1.0;
  }
  // What the frames before left, held toward the mean within the spread.
  let h = resolve_ycocg(textureSampleLevel(history, history_sampler, uv, 0.0));
  let off = h - m1;
  let room = max(spread, vec4f(1e-5));
  let k = max(max(abs(off.x) / room.x, abs(off.y) / room.y), max(abs(off.z) / room.z, abs(off.w) / room.w));
  let held = select(h, m1 + off / k, k > 1.0 && resolve.blend.y > 0.0);
  return resolve_rgb(mix(held, c, blend));
}
