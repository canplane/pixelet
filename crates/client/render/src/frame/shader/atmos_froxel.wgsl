// pxl-render/frame/shader/atmos_froxel.wgsl

// The air's volume (gpu/atmosphere.rs `froxel`; Hillaire 2020, "A Scalable
// and Production Ready Sky and Atmosphere Rendering Technique", its
// aerial-perspective volume): a column of froxels a workgroup and a froxel
// a thread, `AIR_FROXELS` across the screen each way and deep, its depth laid
// over the air its ray crosses (`column_span`, atmos_view.wgsl `air_reach`),
// the march to each froxel written (atmos_view.wgsl `air_held`): what the air
// scatters toward the eye and what it lets through. A slice past them holds
// each column's span, which its reader reads it by (atmos_air.wgsl).

@group(0) @binding(1) var air_out: texture_storage_3d<rgba16float, write>;

// The ray through the middle of column `c`, as the sky's ray forms a pixel's;
// a column past the screen's edge too.
fn column_dir(c: vec2f) -> vec3f {
  let f = f32(AIR_FROXELS);
  let ndc = vec2f((c.x + 0.5) / f * 2.0 - 1.0, 1.0 - (c.y + 0.5) / f * 2.0);
  return normalize(sky_ray() * vec3f(ndc, 1.0));
}

// Column `c`'s span: the spans of it and the columns about it, whole
// (atmos_view.wgsl `air_span`). A point between columns is read off each at
// its own distance, so each column reaches as far as its neighbours' do:
// toward the horizon the sea's plane runs off fast, and a column reaching
// only to its own would leave a point nearer the horizon past its end. As
// the span is held (`air_span_held`), so the march and its reader agree.
fn column_span(c: vec2u) -> vec2f {
  var span = vec2f(3.0e38, 0.0);
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let s = air_span(column_dir(vec2f(c) + vec2f(f32(x), f32(y))));
      span = vec2f(min(span.x, s.x), max(span.y, s.y));
    }
  }
  return air_span_of(air_span_held(span));
}

// The first froxel stands where the column's span starts and the last where
// it ends.
@compute @workgroup_size(1, 1, AIR_FROXELS)
fn froxel_cs(@builtin(global_invocation_id) id: vec3u) {
  let span = column_span(id.xy);
  let z = f32(id.z) / f32(AIR_FROXELS - 1u);
  let dir = column_dir(vec2f(id.xy));
  textureStore(air_out, id, air_held(frame_camera_pos() + dir * air_reach(span, z)));
  if (id.z == 0u) {
    textureStore(air_out, vec3u(id.xy, AIR_FROXELS), vec4f(air_span_held(span), 0.0, 0.0));
  }
}
