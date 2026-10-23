// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a column of froxels a workgroup and a froxel
// a thread, `AIR_FROXELS` across the screen each way and deep, its depth how
// far the air reaches from the nearest anything drawn may lie (atmos_view.wgsl
// `air_reach`), the march to its middle written (atmos_view.wgsl `air_held`):
// what the air scatters toward the eye and what it lets through.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

// The column's froxels' marches, and the air to where the march ends.
var<workgroup> marched: array<vec4f, AIR_FROXELS>;
var<workgroup> ended: vec4f;

// Whether froxel `k` of a column lies before where the march ends, or is the
// first: the march to its middle is what it holds.
fn before_ground(k: u32, ground: f32) -> bool {
  return k == 0u || air_reach((f32(k) + 0.5) / f32(AIR_FROXELS)) <= ground;
}

@compute @workgroup_size(1, 1, AIR_FROXELS)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let n = AIR_FROXELS;
  let f = f32(n);
  let k = id.z;
  // The column's middle through the screen, as the sky's ray forms a pixel's.
  let ndc = vec2f((f32(id.x) + 0.5) / f * 2.0 - 1.0, 1.0 - (f32(id.y) + 0.5) / f * 2.0);
  let dir = normalize(sky_ray() * vec3f(ndc, 1.0));
  let eye = frame_camera_pos();
  let z = (f32(k) + 0.5) / f;
  // Where the march's air ends (`air_ground`): past it the air is the air to
  // there, so a froxel past it holds the line through the last froxel before
  // it and that air, and the volume read at its depth gives that air exactly,
  // which every reader past it reads (atmos_air.wgsl `seen_from_eye`). The
  // first froxel holds the march to its middle whatever lies before it, as
  // the reader fades a point short of it. The line is held as it runs, below
  // zero too; the reader holds what it reads in range.
  let ground = air_ground(dir);
  let mine = before_ground(k, ground);
  if (mine) {
    marched[k] = air_held(eye + dir * air_reach(z));
  } else if (before_ground(k - 1u, ground)) {
    ended = air_held(eye + dir * ground);
  }
  workgroupBarrier();
  if (mine) {
    textureStore(air_out, id, marched[k]);
    return;
  }
  // The last froxel before the ground: the reaches grow with depth.
  var m = k - 1u;
  while (!before_ground(m, ground)) {
    m -= 1u;
  }
  let zk = (f32(m) + 0.5) / f;
  let zs = air_z(ground);
  let line = marched[m] + (ended - marched[m]) * ((z - zk) / max(zs - zk, 1e-6));
  textureStore(air_out, id, select(line, ended, zs - zk < 1e-6));
}
