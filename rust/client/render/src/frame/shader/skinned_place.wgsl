// pxl-render/frame/shader/skinned_place.wgsl

// Places a character's vertices: four-joint skinning from the palette, then the turn from
// glTF's Y-up to Z-up, the heading, and the placement. The lit pass (skinned_mesh.wgsl), the
// depth from the light and the mark (tile_mark.wgsl) all place a vertex through this part, so
// they agree.

// The draw's uniform is read field by field from vec4 words (word.rs `skinned`,
// skinned_draw.slot.wgsl or skinned_draw.uniform.wgsl), never built back into a struct: a
// struct built from them makes Safari's WebGPU lose its GPU process.
fn draw_translation() -> vec3f { return from_eye(skinned_q(), skinned_r()); }

// The palette is read as vec4 columns, because an Android phone's Vulkan driver misreads an
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
  // The draw the vertex belongs to (`draw_at`).
  @builtin(instance_index) instance: u32,
};

fn skin_of(v: VertexIn) -> mat4x4f {
  return joint(v.joints.x) * v.weights.x + joint(v.joints.y) * v.weights.y + joint(v.joints.z) * v.weights.z + joint(v.joints.w) * v.weights.w;
}

fn skinned_world(v: VertexIn, m: mat4x4f) -> vec3f {
  let p = (m * vec4f(v.pos, 1.0)).xyz * skinned_scale();
  return place_up(p, skinned_rot()) + draw_translation();
}

@vertex
fn vs_place(v: VertexIn) -> @builtin(position) vec4f {
  draw_at(v.instance);
  return jittered(frame_view_proj() * vec4f(skinned_world(v, skin_of(v)), 1.0));
}

