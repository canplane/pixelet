// pxl-render/frame/shader/skinned_place.wgsl

// A character's vertices placed: four-joint skinning from the palette, then glTF
// Y-up to Z-up, the heading, and placement; what the lit pass (skinned_mesh.wgsl),
// the depth from the light and the mark (tile_mark.wgsl) all lay alike.

struct Draw {
  // The position's Q3 and its remainder (frame/draw.rs `Position`).
  q: vec3i,
  rot: f32,
  opacity: f32,
  scale: f32,
  _p0: f32,
  _p1: f32,
  r: vec3f,
  _p2: f32,
};

// Read field by field from vec4 words (`draw_word`, skinned_slot.wgsl or
// skinned_mark_draw.wgsl), never as a struct: a struct here lost Safari's
// WebGPU its GPU process.
fn draw_translation() -> vec3f { return from_eye(bitcast<vec3i>(draw_word(0u).xyz), draw_word(2u).xyz); }
fn draw_rot() -> f32 { return draw_word(0u).w; }
fn draw_opacity() -> f32 { return draw_word(1u).x; }
fn draw_scale() -> f32 { return draw_word(1u).y; }

// The palette is read as vec4 columns: a Galaxy S23's Vulkan misread
// `array<mat4x4f>` in storage.
@group(3) @binding(0) var<storage, read> palette: array<vec4f>;

fn joint(j: u32) -> mat4x4f {
  return mat4x4f(palette[j * 4u], palette[j * 4u + 1u], palette[j * 4u + 2u], palette[j * 4u + 3u]);
}

struct VertexIn {
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4f,
  // Which draw it is of (`draw_at`).
  @builtin(instance_index) instance: u32,
};

fn skin_of(v: VertexIn) -> mat4x4f {
  return joint(v.joints.x) * v.weights.x + joint(v.joints.y) * v.weights.y + joint(v.joints.z) * v.weights.z + joint(v.joints.w) * v.weights.w;
}

// One function, so the depth pass places a vertex where the lit pass does.
fn skinned_world(v: VertexIn, m: mat4x4f) -> vec3f {
  let p = (m * vec4f(v.pos, 1.0)).xyz * draw_scale();
  return place_up(p, draw_rot()) + draw_translation();
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  draw_at(v.instance);
  return jittered(frame_view_proj() * vec4f(skinned_world(v, skin_of(v)), 1.0));
}

