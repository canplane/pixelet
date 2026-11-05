// pxl-render/frame/shader/atmos_froxel.wgsl

// A light's share of the air's volume (gpu/atmosphere.rs `froxel`; Hillaire
// 2020, "A Scalable and Production Ready Sky and Atmosphere Rendering
// Technique", its aerial-perspective volume), laid about its light as its sky
// view is (atmos_skyview.wgsl): a column of froxels a workgroup and a froxel
// a thread, `AIR_FROXELS` directions each way, Hillaire's
// (atmos_medium.wgsl `view_unit`), and as many deep by one law for every
// direction (atmos_medium.wgsl `air_dist`), the march along the column
// marched once and each froxel written (`air_held`). Nothing in it is the
// view's, so a turn of the eye keeps it; it is drawn with its light's sky
// view, of the same light and altitude and as it laid them
// (atmos_medium.wgsl `laid`). The sun's half is the first
// `AIR_FROXELS` across, the moon's the next.

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;
// Which light, 1 for the moon, as its sky view's (atmos_skyview.wgsl).
@group(0) @binding(14) var<uniform> light: vec4f;

// The direction through the middle of column `c`, its ends on the first and
// last columns, up +z and the light's azimuth +x, the horizon dipping `beta`.
fn column_dir(beta: f32, c: vec2u) -> vec3f {
  return view_dir(beta, vec2f(c) / f32(AIR_FROXELS - 1u));
}

// What a froxel holds of the air from the eye to it: what it scatters
// toward the eye per share of the light it takes, per channel, S / (1 - T),
// per unit of its light and unexposed, as its sky view holds the sky. The
// air's thickness cancels from it, so it turns slowly across columns and
// slices however fast the air thickens, and a pixel scales it by its own
// ray's 1 - T (atmos_air.wgsl `air_before`). Before the air takes anything
// it holds none.
fn air_held(v: Veil) -> vec4f {
  let taken = vec3f(1.0) - v.transmittance;
  return vec4f(select(vec3f(0.0), v.scattered / taken, taken > vec3f(1e-6)), 1.0);
}

var<workgroup> column_transmittance: array<vec3f, AIR_FROXELS>;
var<workgroup> column_scattered: array<vec3f, AIR_FROXELS>;
var<workgroup> column_held: array<vec4f, AIR_FROXELS>;

// A column marched once: each thread its stretch from the slice before to
// its own (atmos_march.wgsl `gather_between`), then the stretches joined
// from the eye in five doublings, so every froxel is the one integral's to
// its depth and no slice errs apart from the next.
//
// A froxel before its ray enters the air takes nothing and holds nothing:
// it holds the light of the first froxel past it in its column instead, so
// the volume runs on smooth into the air and its read does not blur a step
// from naught there (atmos_air.wgsl `air_held_of`). What the pixel scatters
// before the air is naught still, by its own 1 - T.
@compute @workgroup_size(1, 1, AIR_FROXELS)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let r = camera_r();
  let light_k = u32(light.x);
  let l = laid(light_k);
  let toward = view_light(light_dir(light_k).z);
  let dir = column_dir(l.x, id.xy);
  let e = l.yz;
  let last = f32(AIR_FROXELS - 1u);
  let k = id.z;
  let t0 = select(0.0, air_dist((f32(k) - 1.0) / last, e), k > 0u);
  let t1 = air_dist(f32(k) / last, e);
  let v = gather_between(r, vec3f(0.0, 0.0, 1.0), dir, t0, t1, toward, toward, 0.0, ATMOS_AIR_STRETCH_KNOTS);
  var transmittance = v.transmittance;
  var scattered = v.scattered;
  for (var step = 1u; step < AIR_FROXELS; step <<= 1u) {
    column_transmittance[k] = transmittance;
    column_scattered[k] = scattered;
    workgroupBarrier();
    if (k >= step) {
      let before = column_transmittance[k - step];
      scattered = column_scattered[k - step] + before * scattered;
      transmittance = before * transmittance;
    }
    workgroupBarrier();
  }
  let held = air_held(Veil(transmittance, scattered));
  column_held[k] = held;
  workgroupBarrier();
  var out = held;
  for (var j = k; j < AIR_FROXELS && all(out.rgb == vec3f(0.0)); j++) {
    out = column_held[j];
  }
  textureStore(air_out, vec3u(id.x + light_k * AIR_FROXELS, id.y, k), out);
}
