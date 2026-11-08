// pxl-render/frame/shader/screen_finish.wgsl

// Finishes the canvas from the world's color (gpu/finish.rs). The world's passes fill every
// pixel and leave their color toned and blended in linear light, and the world's own color is
// gathered over frames (screen_resolve.wgsl). This pass brings that color from the world's
// pixels up to the canvas's and encodes it, once (finish.wgsl `finish`). It carries the world's
// depth up with it: each canvas pixel takes the nearest depth of the world texels it covers,
// because what is drawn over the world is tested against that depth. Where the two sizes are
// equal, each pixel takes its own world texel.
//
// The pass draws only the claimed tiles, at the canvas's pixels (screen_tiles.wgsl). It writes
// one color to the canvas and, at location 1, to the picture the world's surface keeps. A
// pipeline whose only target is the canvas leaves the second unwritten.

@group(0) @binding(1) var world: texture_2d<f32>;
@group(0) @binding(2) var world_depth: texture_depth_2d;

struct FinishOut {
  @location(0) color: vec4f,
  @location(1) picture: vec4f,
  @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(v: VertexOut) -> FinishOut {
  let size = frame_viewport();
  let k = size / frame_screen();
  let last = vec2i(size) - 1;
  // The color blended from the four world texels around the pixel's middle.
  let at = v.clip.xy * k - 0.5;
  let i = vec2i(floor(at));
  let f = at - floor(at);
  let c = mix(
    mix(textureLoad(world, clamp(i, vec2i(0), last), 0), textureLoad(world, clamp(i + vec2i(1, 0), vec2i(0), last), 0), f.x),
    mix(textureLoad(world, clamp(i + vec2i(0, 1), vec2i(0), last), 0), textureLoad(world, clamp(i + vec2i(1, 1), vec2i(0), last), 0), f.x),
    f.y,
  );
  // The nearest depth of the world texels the pixel spans, which is the greatest, as the depth
  // is reversed.
  let lo = clamp(vec2i(floor((v.clip.xy - 0.5) * k)), vec2i(0), last);
  let hi = clamp(vec2i(ceil((v.clip.xy + 0.5) * k) - 1.0), lo, last);
  var d = 0.0;
  for (var y = lo.y; y <= hi.y; y++) {
    for (var x = lo.x; x <= hi.x; x++) {
      d = max(d, textureLoad(world_depth, vec2i(x, y), 0));
    }
  }
  var o: FinishOut;
  o.color = vec4f(finish(c.rgb, v.clip.xy), 1.0);
  o.picture = o.color;
  o.depth = d;
  return o;
}
