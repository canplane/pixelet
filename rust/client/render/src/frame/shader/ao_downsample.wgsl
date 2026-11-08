// Downsamples the depth to half resolution, one float a texel, which every tap
// of the sample, the denoise and the apply reads. A texel keeps the nearest of
// its 2×2 pixels or the farthest, alternating on a checkerboard, so an edge
// keeps both of its sides. The sample (ao_sample.wgsl) finds the normal from
// the full-resolution depth at the pixel kept here.

@group(0) @binding(1) var scene_depth: texture_depth_2d;

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(frame_viewport()) - 1), 0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let hp = vec2i(i.clip.xy);
  let fp = hp * i32(AO_DOWNSCALE);
  let d = vec4f(
    depth_at(fp),
    depth_at(fp + vec2i(1, 0)),
    depth_at(fp + vec2i(0, 1)),
    depth_at(fp + vec2i(1, 1)),
  );
  // The depth is reversed, so the greater value is the nearer.
  let nearest = max(max(d.x, d.y), max(d.z, d.w));
  let farthest = min(min(d.x, d.y), min(d.z, d.w));
  return vec4f(select(nearest, farthest, (hp.x + hp.y) % 2 == 1), 0.0, 0.0, 0.0);
}
