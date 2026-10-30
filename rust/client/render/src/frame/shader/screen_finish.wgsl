// pxl-render/frame/shader/screen_finish.wgsl

// The world's color, toned and blended linear by its passes and the world's
// gathered over frames (screen_resolve.wgsl), brought from the world's
// pixels up to the canvas's and encoded onto it once (finish.wgsl
// `finish`), premultiplied by its alpha for a surface that shows through
// where nothing was drawn; and the world's depth carried up with it, the
// nearest of what a canvas pixel covers, which what is drawn over the world
// is tested against. Where the two sizes are one, each is the world's own
// texel. The claimed tiles alone, at the canvas's pixels (screen_tiles.wgsl),
// into the picture the surface keeps.

@group(0) @binding(1) var world: texture_2d<f32>;
@group(0) @binding(2) var world_depth: texture_depth_2d;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

struct FinishOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}

@fragment
fn fs(v: VertexOut) -> FinishOut {
  let size = frame_viewport();
  let k = size / frame_screen();
  let last = vec2i(size) - 1;
  // The color between the four world texels about the pixel's middle.
  let at = v.clip.xy * k - 0.5;
  let i = vec2i(floor(at));
  let f = at - floor(at);
  let c = mix(
    mix(textureLoad(world, clamp(i, vec2i(0), last), 0), textureLoad(world, clamp(i + vec2i(1, 0), vec2i(0), last), 0), f.x),
    mix(textureLoad(world, clamp(i + vec2i(0, 1), vec2i(0), last), 0), textureLoad(world, clamp(i + vec2i(1, 1), vec2i(0), last), 0), f.x),
    f.y,
  );
  // The nearest depth, the greatest reversed, of the world texels the pixel
  // spans.
  let lo = clamp(vec2i(floor((v.clip.xy - 0.5) * k)), vec2i(0), last);
  let hi = clamp(vec2i(ceil((v.clip.xy + 0.5) * k) - 1.0), lo, last);
  var d = 0.0;
  for (var y = lo.y; y <= hi.y; y++) {
    for (var x = lo.x; x <= hi.x; x++) {
      d = max(d, textureLoad(world_depth, vec2i(x, y), 0));
    }
  }
  var o: FinishOut;
  // Blended premultiplied, as its passes left it: encoded straight, then
  // premultiplied again, so an edge brought up keeps its color.
  let straight = select(c.rgb / max(c.a, 1e-6), c.rgb, c.a >= 1.0);
  o.color = vec4f(finish(straight, v.clip.xy) * c.a, c.a);
  o.depth = d;
  return o;
}
