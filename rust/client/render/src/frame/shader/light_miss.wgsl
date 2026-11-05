// pxl-render/light_miss.wgsl

// The slots `light_miss_hit_cs` left (pxl-engine `light::misses_left_at`), their walk on from
// where it left them, which each slot's word holds, asking the world (ask.wgsl,
// light_resume.wgsl).

const WALK_PASS: u32 = LIGHT_MISSED_PASS;
const VISITS_COUNT: u32 = LIGHT_MISS_COUNT;
const ASKED_COUNT: u32 = LIGHT_ASKED_COUNT;

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;

@compute @workgroup_size(MISS_LANES)
fn light_miss_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let at = head_word(LIGHT_HEAD_MISSES_LEFT);
  let k = list_place(group.xy, lane.x);
  if (k >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + k];
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let texel = slot_texel(slot);
  let aim = light_aim(texel, 0u);
  let t = bitcast<f32>(textureLoad(light, slot, 0).x);
  let w = walk_asking(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, light_resumed_at(aim, texel, t));
  textureStore(light, slot, 0, vec4<u32>(walked_word(w), 0u, 0u, 0u));
}
