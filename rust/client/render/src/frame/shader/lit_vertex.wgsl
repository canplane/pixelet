// pxl-render/frame/shader/lit_vertex.wgsl

// What a lit model's vertex hands its fragment, decor's, an impostor's, a
// character's and the location beacon's: where it stands, the air before it and
// the light on it, each worked out at the vertex (atmos_air.wgsl `seen_from_eye`,
// atmos_view.wgsl `light_of`), and the one word of its draw the fragment reads,
// `draw`, a model's opacity and the beacon's night.

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

// A model's `albedo` lit at `i`, at its opacity.
fn lit_color(albedo: vec3f, i: VertexOut) -> vec4f {
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, lit_air(i), lit_light(i));
  return vec4f(tone(color), i.draw);
}
