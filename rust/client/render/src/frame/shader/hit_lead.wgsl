// pxl-render/hit_lead.wgsl

// Each hit the walks again kept (miss_hit.wgsl `keep_hit`), a thread each, dispatched off
// their count (pxl-engine `pyramid::HITS_ARGS`): the one that asks what lies about its face
// is one whose pixel's neighbors to the left and above wait for no hit alike within its
// square of `HIT_SQUARE` a side, so a patch of hits alike is asked once a square, and which
// of them asks is a function of the pixels alone. It is listed with its ask for the faces'
// pass as a box is (pxl-shader raster.rs `face_cs`), off the line its cell's rays resumed
// from. Any other is told the neighbor's hit it takes them from, the left before the one
// above (`beam::HIT_FOLLOWS`), which `hit_faces_cs` follows. It reads the hits and its
// neighbors' last ray words, and writes only its own hit's last word and its ask, so no
// thread reads what another writes (pxl-shader cast.rs `hit_lead_cs`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read> pyramid: array<u32>;
/// The walk again's words, the faces' pass's count and dispatch among them
/// (`ray::MISS_ARGS_ROOM`).
@group(0) @binding(14) var<storage, read_write> marks: array<atomic<u32>>;

/// Whether the hits at words `a` and `b` are one: one node entered through one face.
fn alike(a: u32, b: u32) -> bool {
  for (var k = 0u; k < HIT_PX; k += 1u) {
    if (beams[a + k] != beams[b + k]) {
      return false;
    }
  }
  return true;
}

/// The number of the hit pixel `p` waits for, where it is one alike the hit at words `k`;
/// `HITS` where not.
fn alike_hit(k: u32, p: vec2<u32>) -> u32 {
  let w = textureLoad(rays, p, RAY_FACES).x;
  if ((w & HIT_WAITS) == 0u) {
    return HITS;
  }
  let r = w & ~HIT_WAITS;
  if (r >= HITS || !alike(hit_at(r), k)) {
    return HITS;
  }
  return r;
}

@compute @workgroup_size(HIT_LANES)
fn hit_lead_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= min(pyramid[HITS_CNT], HITS)) {
    return;
  }
  let k = hit_at(id.x);
  let px = beams[k + HIT_PX];
  let p = vec2<u32>(px & 0xffffu, px >> 16u);
  var follows = HITS;
  if (p.x % HIT_SQUARE != 0u) {
    follows = alike_hit(k, p - vec2<u32>(1u, 0u));
  }
  if (follows == HITS && p.y % HIT_SQUARE != 0u) {
    follows = alike_hit(k, p - vec2<u32>(0u, 1u));
  }
  if (follows != HITS) {
    beams[k + HIT_OUT] = HIT_FOLLOWS | follows;
    return;
  }
  // Its place among the asks, and the faces' pass's workgroups raised by the place that
  // opens one.
  let place = atomicAdd(&marks[MISS_ARGS_ROOM + RASTER_ARGS_INSTANCES], 1u);
  if (place % RASTER_FACES_LANES == 0u) {
    atomicMax(&marks[MISS_ARGS_ROOM + RASTER_ARGS_FACES], place / RASTER_FACES_LANES + 1u);
  }
  let cell = grid_cell(px_tan(p, textureDimensions(rays)));
  let a = eye_word(GRID + GRID_HIT_ASKS_AT) + place * ASK_WORDS;
  beams[a + ASK_NODE] = k + HIT_NODE;
  beams[a + ASK_FACE] = beams[k + HIT_FACE];
  beams[a + ASK_LINE] = beams[cell_start(cell) + CELL_CORNER] * LINE_WORDS;
  beams[a + ASK_TO] = k + HIT_OUT;
}
