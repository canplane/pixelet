// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a column of froxels a thread, `AIR_FROXELS`
// across the screen each way and deep, its depth how far the air reaches from
// the nearest anything drawn may lie (atmos_view.wgsl `air_reach`), the march
// to its middle written (atmos_view.wgsl `air_held`): what the air scatters
// toward the eye and what it lets through.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let n = AIR_FROXELS;
  if (any(id.xy >= vec2u(n))) {
    return;
  }
  let f = f32(n);
  // The column's middle through the screen, as the sky's ray forms a pixel's.
  let ndc = vec2f((f32(id.x) + 0.5) / f * 2.0 - 1.0, 1.0 - (f32(id.y) + 0.5) / f * 2.0);
  let dir = normalize(sky_ray() * vec3f(ndc, 1.0));
  let eye = frame_camera_pos();
  // Where the march's air ends (`air_ground`): past it the air is the air to
  // there, so a froxel past it holds the line through the last froxel before
  // it and that air, and the volume read at its depth gives that air exactly,
  // which every reader past it reads (atmos_air.wgsl `seen_from_eye`). The
  // first froxel holds the march to its middle whatever lies before it, as
  // the reader fades a point short of it.
  let ground = air_ground(dir);
  let zs = air_z(ground);
  var before = vec4f(0.0, 0.0, 0.0, 1.0);
  var zk = 0.0;
  var at = vec4f(0.0);
  var met = false;
  for (var k = 0u; k < n; k++) {
    let z = (f32(k) + 0.5) / f;
    let reach = air_reach(z);
    var held: vec4f;
    if (k == 0u || reach <= ground) {
      held = air_held(eye + dir * reach);
      before = held;
      zk = z;
    } else {
      if (!met) {
        at = air_held(eye + dir * ground);
        met = true;
      }
      let line = before + (at - before) * ((z - zk) / max(zs - zk, 1e-6));
      held = select(vec4f(max(line.rgb, vec3f(0.0)), clamp(line.a, 0.0, 1.0)), at, zs - zk < 1e-6);
    }
    textureStore(air_out, vec3u(id.xy, k), held);
  }
}
