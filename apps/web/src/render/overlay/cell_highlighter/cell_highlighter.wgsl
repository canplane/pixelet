// @pxl/web/render/overlay/cell_highlighter/cell_highlighter.wgsl

struct Uniforms {
  view_proj: mat4x4<f32>,
  world_info: vec4<f32>,
  local_world_offset: vec4<f32>,
  _pad0: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VSIn {
  @location(0) position: vec3<f32>,
  @location(1) pos_scale: vec4<f32>,
  @location(2) color: vec4<f32>,
};

struct VSOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) unit_pos: vec3<f32>,
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
fn vs_main(in: VSIn) -> VSOut {
  let local = in.position * in.pos_scale.w + in.pos_scale.xyz + u.local_world_offset.xyz;
  let world = rotate_z(local, u.world_info.x);

  var out: VSOut;
  out.position = u.view_proj * vec4<f32>(world, 1.0);
  out.color = in.color;
  out.unit_pos = in.position;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let ax = abs(in.unit_pos.x);
  let ay = abs(in.unit_pos.y);
  let az = abs(in.unit_pos.z);

  let edge = max(max(ax, ay), az);
  let border = select(0.0, 1.0, edge > 0.45);

  let fill_alpha = in.color.a;
  let alpha = fill_alpha + (1.0 - fill_alpha) * border;

  return vec4<f32>(in.color.rgb, alpha);
}