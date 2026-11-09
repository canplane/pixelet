// pxl-render/light_mark.wgsl

// Stands the world's receivers of the shadow map that the eye walked apart on the tiles their
// hits lie on. Every receiver stands by one rule (pxl-engine `Light::box_tiles`): a box within
// the effects' reach (`Light::reach`) stands on the tiles its corners lie on, and on the
// footprint about them where the head asks for it (`light::HEADER_FOOTPRINT`). Each drawn
// pixel did one of three things. It took a drawable entry's voxel, whose box the raster drew,
// and so hit within that box, which the record's draw stands as it writes it (record_draw.wgsl
// `record_draw_cs`). It met the sky, which receives nothing. Or it was walked apart, and its
// walked bit is set (cast.wgsl `cast_cs`, walked.wgsl). So only the walked pixels' hits stand
// here, each as a box of one point, one thread for each word of the walked bits
// (`light_mark_walked_cs`). The walked bits lie past the grid's words among the eye's beam
// rays' words. The pass runs after the eye's rays and its record's draw, and before the drawn
// receivers (light_mark_draws.wgsl) and the propagation (light_propagate.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// The eye's beam rays' words, which hold its walked bits (walked.wgsl).
@group(0) @binding(23) var<storage, read> eye_beams: array<u32>;

fn eye_beam(i: u32) -> u32 {
  return select(0u, eye_beams[i], i < arrayLength(&eye_beams));
}

/// Stands the hit of each pixel whose bit is set in the word of walked bits at the thread's
/// place, as a box of one point. A pixel whose ray met nothing has a first word of 0 and stands
/// on nothing (pxl-engine `ray::first_word`).
@compute @workgroup_size(MISS_LANES, 1, 1)
fn light_mark_walked_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = list_place(group.xy, lane.x);
  let size = textureDimensions(rays);
  if (k >= walked_words(size)) {
    return;
  }
  var bits = eye_beam(walked_at() + k);
  while (bits != 0u) {
    let p = bit_px(k, firstTrailingBit(bits), size);
    bits = bits & (bits - 1u);
    if (any(p >= size)) {
      continue;
    }
    let first = textureLoad(rays, p, 0u).x;
    if (first == 0u) {
      continue;
    }
    let t = bitcast<f32>(textureLoad(rays, p, 1u).x);
    let at = eye_aim(px_tan(p, size)).dir * t;
    let b = box_tiles(at, at);
    if (b.has_tiles) {
      stand_box(b.first, b.last);
    }
  }
}
