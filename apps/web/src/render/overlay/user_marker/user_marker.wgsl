// @pxl/web/render/overlay/user_marker/user_marker.wgsl

struct Uniforms {
  view_proj: mat4x4<f32>,
  world_info: vec4<f32>,
  local_world_offset: vec4<f32>,
  _pad0: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct BeaconVSIn {
  @location(0) position: vec3<f32>,
  @location(1) pos_scale: vec4<f32>,
  @location(2) color: vec4<f32>,
};

struct BeaconVSOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

struct ZoneVSIn {
  @location(0) position: vec3<f32>,
  @location(1) pos_scale: vec4<f32>,
  @location(2) color: vec4<f32>,
  @location(3) borders: vec4<f32>,
};

struct ZoneVSOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) unit_pos: vec3<f32>,
  @location(2) borders: vec4<f32>,
};

fn rotate_z(v: vec3<f32>, azimuth: f32) -> vec3<f32> {
  let c = cos(azimuth);
  let s = sin(azimuth);
  return vec3<f32>(
    c * v.x - s * v.y,
    s * v.x + c * v.y,
    v.z,
  );
}

@vertex
fn beacon_vs_main(in: BeaconVSIn) -> BeaconVSOut {
  let local = in.position * in.pos_scale.w + in.pos_scale.xyz + u.local_world_offset.xyz;
  let world = rotate_z(local, u.world_info.x);

  var out: BeaconVSOut;
  out.position = u.view_proj * vec4<f32>(world, 1.0);
  out.color = in.color;
  return out;
}

@fragment
fn beacon_fs_main(in: BeaconVSOut) -> @location(0) vec4<f32> {
  return in.color;
}

@vertex
fn zone_vs_main(in: ZoneVSIn) -> ZoneVSOut {
  let local = in.position * in.pos_scale.w + in.pos_scale.xyz + u.local_world_offset.xyz;
  let world = rotate_z(local, u.world_info.x);

  var out: ZoneVSOut;
  out.position = u.view_proj * vec4<f32>(world, 1.0);
  out.color = in.color;
  out.unit_pos = in.position;
  out.borders = in.borders;
  return out;
}

@fragment
fn zone_fs_main(in: ZoneVSOut) -> @location(0) vec4<f32> {
  let border_width = 0.085;
  let ux = in.unit_pos.x + 0.5;
  let uy = in.unit_pos.y + 0.5;

  let top = select(0.0, 1.0, uy > (1.0 - border_width)) * in.borders.x;
  let bottom = select(0.0, 1.0, uy < border_width) * in.borders.y;
  let right = select(0.0, 1.0, ux > (1.0 - border_width)) * in.borders.z;
  let left = select(0.0, 1.0, ux < border_width) * in.borders.w;

  let border = max(max(top, bottom), max(right, left));
  let fill_alpha = in.color.a;
  let alpha = fill_alpha + (1.0 - fill_alpha) * border;

  return vec4<f32>(in.color.rgb, alpha);
}