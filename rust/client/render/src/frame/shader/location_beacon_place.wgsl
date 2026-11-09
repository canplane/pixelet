// pxl-render/frame/shader/location_beacon_place.wgsl

// Places the beacon's octahedron, spun about Z. The lit pass
// (location_beacon.wgsl) and the mark (tile_mark.wgsl) both place it here, so
// the mark covers the pixels the beacon draws.

fn beacon_world(pos: vec3f) -> vec3f {
  return spin_z(pos, location_spin()) + draw_translation();
}

@vertex
fn vs_place(@location(0) pos: vec3f, @location(1) normal: vec3f) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(beacon_world(pos), 1.0));
}
