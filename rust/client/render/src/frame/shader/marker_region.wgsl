// pxl-render/frame/shader/marker_region.wgsl

// Each marker's region at its twin's depth, as its number in four bytes,
// where the world leaves its anchor seen (marker.wgsl).

@group(2) @binding(1) var world_depth: texture_depth_2d;

struct RegionOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) number: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> RegionOut {
  let p = placed(vi, ii, true);
  var o: RegionOut;
  o.number = marker_number(ii);
  o.clip = vec4f(p.clip.xy, p.upright_z * p.clip.w, p.clip.w);
  let at = anchor_pixel(p, textureDimensions(world_depth));
  if (at.x >= 0) {
    // Reversed depth: nearer is greater.
    let probe = marker_proj() * vec4f(p.anchor_view + vec3f(0.0, 0.0, p.push), 1.0);
    if (textureLoad(world_depth, at, 0) > probe.z / probe.w) {
      o.clip = NOWHERE;
    }
  }
  return o;
}

@fragment
fn fs(i: RegionOut) -> @location(0) vec4f {
  return vec4f(unpack4x8unorm(i.number));
}
