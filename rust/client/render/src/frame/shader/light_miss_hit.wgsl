// pxl-render/light_miss_hit.wgsl

// The slots `light_cs` listed, their walk from the root over the node cache alone
// (walk.wgsl). One that ends writes its word and is struck from the list; `light_miss_cs`
// walks what is left (light_miss.wgsl).

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;

/// `light_miss_cs`'s dispatch: its rows, up to the last it has to walk.
@group(0) @binding(13) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(MISS_LANES)
fn light_miss_hit_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let misses = head_word(LIGHT_HEAD_MISSES);
  let cnt = beams[head_word(LIGHT_HEAD_MISSES_CNT)];
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  if (k >= cnt) {
    return;
  }
  let listed = beams[misses + k];
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let texel = slot_texel(slot);
  let w = walk(
    light_aim(texel, mix_hash(head_word(LIGHT_HEAD_CAST), listed)),
    root_start(),
    NONE,
  );
  if (w.ends == 4u) {
    atomicMax(&left[1], group.y + 1u);
    return;
  }
  textureStore(light, slot, 0, vec4<u32>(walked_word(w), 0u, 0u, 0u));
  beams[misses + k] = LIGHT_MISS_STRUCK;
}
