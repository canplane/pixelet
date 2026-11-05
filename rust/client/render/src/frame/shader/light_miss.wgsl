// pxl-render/light_miss.wgsl

// The slots `light_miss_hit_cs` left, their walk from the root, asking the world (ask.wgsl).

const WALK_PASS: u32 = LIGHT_MISSED_PASS;
const VISITS_COUNT: u32 = LIGHT_MISS_COUNT;
const ASKED_COUNT: u32 = LIGHT_ASKED_COUNT;

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(12) var<storage, read> list: array<u32>;

@compute @workgroup_size(MISS_LANES)
fn light_miss_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let misses = head_word(LIGHT_HEAD_MISSES);
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  if (k >= list[misses]) {
    return;
  }
  let listed = list[misses + 1u + k];
  if (listed == LIGHT_MISS_STRUCK) {
    return;
  }
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let texel = slot_texel(slot);
  let w = walk_asking(
    texel_start(light_corner() + vec2<i32>(texel)),
    light_v3(LIGHT_DIR),
    eye_f32(LIGHT + LIGHT_T_MAX),
    vec2<u32>(texel_band(texel)),
    root_start(),
  );
  textureStore(light, slot, 0, vec4<u32>(walked_word(w), 0u, 0u, 0u));
}
