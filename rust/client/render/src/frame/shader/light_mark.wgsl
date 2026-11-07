// pxl-render/light_mark.wgsl

// The world's receivers of the shadow map that the eye walked apart, stood on by the tiles
// their hits lie on, by the one rule every receiver stands by (pxl-engine `Light::box_tiles`):
// a box within the effects' reach (`Light::reach`) stands on the tiles its corners lie on, and
// the footprint about them where the head asks it (`light::HEAD_FOOTPRINT`). A drawn pixel
// took a drawable entry's voxel whose box the raster drew, so hit within that box, which
// the record's draw stands as it writes it (record_draw.wgsl `record_draw_cs`); met the sky,
// which receives nothing; or was walked apart, its walked bit set (cast.wgsl `cast_cs`,
// walked.wgsl). So every walked pixel's hit stands here, a box of one point, a thread a word of
// the walked bits (`light_mark_walked_cs`), past the grid's words among the eye's beam rays'. Run after the eye's rays and its
// record's draw, before the drawn receivers (light_mark_draws.wgsl) and the propagation
// (light_propagate.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The eye's beam rays' words, its walked bits among them (walked.wgsl).
@group(0) @binding(23) var<storage, read> eye_beams: array<u32>;

fn eye_beam(i: u32) -> u32 {
  return select(0u, eye_beams[i], i < arrayLength(&eye_beams));
}

/// Each pixel the eye's cast walked apart, of the word of walked bits at the thread's place:
/// its hit, a box of one point, where its ray met the world; none where it met nothing
/// (pxl-engine `ray::first_word`).
@compute @workgroup_size(MISS_LANES, 1, 1)
fn light_mark_walked_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = list_place(group.xy, lane.x);
  let drawn = textureDimensions(rays);
  if (k >= walked_words(drawn)) {
    return;
  }
  var bits = eye_beam(walked_at() + k);
  while (bits != 0u) {
    let p = bit_px(k, firstTrailingBit(bits), drawn);
    bits = bits & (bits - 1u);
    if (any(p >= drawn)) {
      continue;
    }
    let first = textureLoad(rays, p, 0u).x;
    if (first == 0u) {
      continue;
    }
    let t = bitcast<f32>(textureLoad(rays, p, 1u).x);
    let at = eye_aim(px_tan(p, drawn)).dir * t;
    let b = box_tiles(at, at);
    if (b.got) {
      stand_box(b.first, b.last);
    }
  }
}
