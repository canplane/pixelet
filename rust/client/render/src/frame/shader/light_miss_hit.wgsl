// pxl-render/light_miss_hit.wgsl

// The slots `light_cs` listed, their walk from their cell's start over the node cache alone
// (walk.wgsl, cached.wgsl, light_resume.wgsl). One that ends writes its word; one it leaves is listed for
// `light_miss_cs` (light_miss.wgsl), which walks only those (pxl-engine `light::misses_left_at`)
// on from where it was left, which its slot's word holds until then (`resumed_at`).

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;

/// `light_miss_cs`'s dispatch, then how many are listed (list.wgsl `Left`).
@group(0) @binding(13) var<storage, read_write> args: Left;

@compute @workgroup_size(MISS_LANES)
fn light_miss_hit_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  // Both lists hold a place for each slot (pxl-engine `light::misses_at`, `misses_left_at`).
  let size = head_word(LIGHT_HEAD_SIZE);
  let cnt = in_room(beams[head_word(LIGHT_HEAD_MISSES_CNT)], size * size);
  let k = list_place(group.xy, lane.x);
  if (k >= cnt) {
    return;
  }
  let listed = beams[head_word(LIGHT_HEAD_MISSES) + k];
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let texel = slot_texel(slot);
  let aim = light_aim(texel);
  let w = walk(aim, light_resumed(aim, texel), STEPS_MOST);
  if (w.ends == 4u) {
    textureStore(light, slot, 0, vec4<u32>(bitcast<u32>(w.t), 0u, 0u, 0u));
    let place = enlist(size * size);
    if (place < size * size) {
      beams[head_word(LIGHT_HEAD_MISSES_LEFT) + 1u + place] = listed;
    }
    return;
  }
  textureStore(light, slot, 0, vec4<u32>(walked_word(w), 0u, 0u, 0u));
}
