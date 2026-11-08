// pxl-render/frame/shader/lit_vertex.wgsl

// What the vertex of a lit model hands its fragment. Decor, impostors,
// characters and the location beacon are drawn through it. The air between the
// eye and the vertex (atmos_air.wgsl `seen_from_eye`) and the light on the
// vertex (atmos_view.wgsl `light_of`) are worked out at the vertex, not at each
// fragment. The fragment reads one word of its draw, `draw`: a model's opacity,
// or whether it is night for the beacon.

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) transmittance: f32,
  @location(4) scattered: vec3f,
  @location(5) @interpolate(flat) fill: vec3f,
  @location(6) @interpolate(flat) sun: vec3f,
  @location(7) @interpolate(flat) other: vec3f,
  @location(8) @interpolate(flat) draw: f32,
};

fn lit_vertex(world: vec3f, uv: vec2f, normal: vec3f, draw: f32) -> VertexOut {
  let clip = jittered(frame_view_proj() * vec4f(world, 1.0));
  let seen = seen_from_eye(world);
  let l = light_of(world);
  return VertexOut(clip, uv, normal, world, seen.transmittance, seen.scattered, l.fill, l.sun, l.other, draw);
}

fn lit_air(i: VertexOut) -> Air {
  return Air(i.transmittance, i.scattered);
}

fn lit_light(i: VertexOut) -> Light {
  return Light(i.sun, i.other, i.fill);
}

// A model's `albedo` lit at `i` and toned, with the model's opacity as alpha.
fn lit_color(albedo: vec3f, i: VertexOut) -> vec4f {
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, lit_air(i), lit_light(i));
  return vec4f(tone(color), i.draw);
}
