// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a froxel a thread, `AIR_FROXELS` across the
// screen each way and deep, its depth's share squared of how far the air
// reaches (atmos_air.wgsl `air_far`), the march to its middle
// (atmos_view.wgsl `seen_at`) written as its four terms, a depth of froxels
// each: what the air scatters toward the eye and what it lets through, then
// the sun's, the other light's and the sky's light there.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let n = AIR_FROXELS;
  if (any(id >= vec3u(n))) {
    return;
  }
  let f = f32(n);
  // The froxel's middle through the screen, as the sky's ray forms a pixel's.
  let ndc = vec2f((f32(id.x) + 0.5) / f * 2.0 - 1.0, 1.0 - (f32(id.y) + 0.5) / f * 2.0);
  let dir = normalize(sky_ray() * vec3f(ndc, 1.0));
  let z = (f32(id.z) + 0.5) / f;
  let s = seen_at(frame_camera_pos() + dir * (air_far() * z * z));
  textureStore(air_out, id, vec4f(s.scattered, s.through));
  textureStore(air_out, id + vec3u(0u, 0u, n), vec4f(s.light.sun, 0.0));
  textureStore(air_out, id + vec3u(0u, 0u, 2u * n), vec4f(s.light.other, 0.0));
  textureStore(air_out, id + vec3u(0u, 0u, 3u * n), vec4f(s.light.fill, 0.0));
}
