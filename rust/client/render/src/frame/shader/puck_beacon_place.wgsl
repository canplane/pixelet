// pxl-render/frame/shader/puck_beacon_place.wgsl

// The beacon's octahedron placed, spun about Z: what the lit pass
// (puck_beacon.wgsl) and the mark (tile_mark.wgsl) lay alike.

fn beacon_world(pos: vec3f) -> vec3f {
  return spin_z(pos, draw_fourth()) + draw_translation();
}

@vertex
fn vs_depth(@location(0) pos: vec3f, @location(1) normal: vec3f) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(beacon_world(pos), 1.0));
}
