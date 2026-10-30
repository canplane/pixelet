// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a column of froxels a workgroup and a froxel
// a thread, `AIR_FROXELS` across the screen each way and deep, its depth laid
// by one law for every column (atmos_view.wgsl `air_reach`), the march to
// the column marched once and each froxel written (atmos_view.wgsl
// `air_held`): what the air scatters toward the eye per share of the light
// it takes.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

// The ray through the middle of column `c`, as the sky's ray forms a pixel's;
// a column past the screen's edge too.
fn column_dir(c: vec2f) -> vec3f {
  let f = f32(AIR_FROXELS);
  let ndc = vec2f((c.x + 0.5) / f * 2.0 - 1.0, 1.0 - (c.y + 0.5) / f * 2.0);
  return normalize(sky_ray() * vec3f(ndc, 1.0));
}

var<workgroup> column_transmittance: array<vec3f, AIR_FROXELS>;
var<workgroup> column_scattered: array<vec3f, AIR_FROXELS>;
var<workgroup> column_held: array<vec4f, AIR_FROXELS>;

// A column marched once: each thread its stretch from the slice before to
// its own (atmos_view.wgsl `air_stretch`), then the stretches joined from
// the eye in five doublings, so every froxel is the one integral's to its
// depth and no slice errs apart from the next.
//
// A froxel before its ray enters the air takes nothing and holds nothing:
// it holds the light of the first froxel past it in its column instead, so
// the volume runs on smooth into the air and its read does not blur a step
// from naught there (atmos_air.wgsl `air_held_at`). What the pixel scatters
// before the air is naught still, by its own 1 - T.
@compute @workgroup_size(1, 1, AIR_FROXELS)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let last = f32(AIR_FROXELS - 1u);
  let k = id.z;
  let t0 = select(0.0, air_reach((f32(k) - 1.0) / last), k > 0u);
  var v = air_stretch(column_dir(vec2f(id.xy)), t0, air_reach(f32(k) / last));
  for (var step = 1u; step < AIR_FROXELS; step <<= 1u) {
    column_transmittance[k] = v.transmittance;
    column_scattered[k] = v.scattered;
    workgroupBarrier();
    if (k >= step) {
      let before = column_transmittance[k - step];
      v = Veil(before * v.transmittance, column_scattered[k - step] + before * v.scattered);
    }
    workgroupBarrier();
  }
  let held = air_held(v);
  column_held[k] = held;
  workgroupBarrier();
  var out = held;
  for (var j = k; j < AIR_FROXELS && all(out.rgb == vec3f(0.0)); j++) {
    out = column_held[j];
  }
  textureStore(air_out, id, out);
}
