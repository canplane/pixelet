// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a column of froxels a workgroup and a froxel
// a thread, `AIR_FROXELS` across the screen each way and deep, its depth laid
// over the air its ray crosses (atmos_view.wgsl `air_span`, `air_reach`), the
// march to each froxel written (atmos_view.wgsl `air_held`): what the air
// scatters toward the eye and what it lets through.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

// The first froxel stands where the column's span starts and the last where
// it ends, so a reader there reads the march to it, and one past the end
// reads the air to the end (atmos_air.wgsl `seen_from_eye`).
@compute @workgroup_size(1, 1, AIR_FROXELS)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let f = f32(AIR_FROXELS);
  // The column's middle through the screen, as the sky's ray forms a pixel's.
  let ndc = vec2f((f32(id.x) + 0.5) / f * 2.0 - 1.0, 1.0 - (f32(id.y) + 0.5) / f * 2.0);
  let dir = normalize(sky_ray() * vec3f(ndc, 1.0));
  let z = f32(id.z) / (f - 1.0);
  textureStore(air_out, id, air_held(frame_camera_pos() + dir * air_reach(air_span(dir), z)));
}
