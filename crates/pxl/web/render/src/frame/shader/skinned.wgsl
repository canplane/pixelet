// Actors: the module's skinned draws (model.rs) against each model's mesh
// block and each actor's joint palette, both as the module wrote them. A
// vertex is skinned by its four joints and weights out of the palette,
// scaled to the model's height, turned from glTF's Y-up to this world's
// Z-up and by the actor's heading about Z, and placed; the fragment is
// decor's recipe.

struct Draw {
  translation: vec3f,
  rotation: f32,
  opacity: f32,
  scale: f32,
  _p0: f32,
  _p1: f32,
};

// Held as vec4 words and read back by `draw()`: what the host and a shader
// share has no room inside it for a driver to lay out its own way
// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw() -> Draw {
  return Draw(
    draw_words[0].xyz,
    draw_words[0].w,
    draw_words[1].x,
    draw_words[1].y,
    draw_words[1].z,
    draw_words[1].w,
  );
}

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
// The palette is mat4s, one a joint, read as the four vec4 columns each is: a
// Galaxy S23's Vulkan read an `array<mat4x4f>` in storage as some other
// layout, and a vec4 has no layout to read two ways
// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
@group(3) @binding(0) var<storage, read> palette: array<vec4f>;

fn joint(j: u32) -> mat4x4f {
  return mat4x4f(palette[j * 4u], palette[j * 4u + 1u], palette[j * 4u + 2u], palette[j * 4u + 3u]);
}

struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  // The air in front of the vertex (atmos_view.wgsl's Seen).
  @location(3) through: vec3f,
  @location(4) scattered: vec3f,
  @location(5) past: vec3f,
  @location(6) fill: vec3f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let m = joint(v.joints.x) * v.weights.x + joint(v.joints.y) * v.weights.y + joint(v.joints.z) * v.weights.z + joint(v.joints.w) * v.weights.w;
  let p = (m * vec4f(v.position, 1.0)).xyz * draw().scale;
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  let world = on_globe(place_up(p, draw().rotation) + draw().translation);
  var o: VertexOut;
  o.clip = frame().view_proj * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place_up(n, draw().rotation);
  o.world_pos = world;
  let seen = seen_at_vertex(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.past = seen.past;
  o.fill = seen.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = screen_ao(shade(albedo, n, i.world_pos, sun_lit(i.world_pos, n), vec3f(1.0), Seen(i.through, i.scattered, i.past, i.fill)), i.clip.xy);
  return vec4f(finish(color, i.clip.xy), draw().opacity);
}
