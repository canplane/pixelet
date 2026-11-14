// pxl-render/frame/shader/light_propagate.wgsl

// Lists which tiles of the box a receiver reads, one thread for each tile a receiver stands on
// (`light::LIST_RECEIVERS`; light_mark.wgsl, light_mark_draws.wgsl). It runs after every
// receiver stood and after the slide (light_slide.wgsl), and before the beam. A receiver reads
// the tiles in the disk it reads (`Light::read`) that lie within a tile and its taps of a step
// on the way up from the tile it stands on (`up`, `up_at`). A point that far up lies that many
// tiles over, or one more, and its taps reach a tile either side. Decor stands on its footprint
// (light_mark_draws.wgsl) and the characters on what the eye meets, and neither is taller than
// the height the way up climbs to. A tile is listed as read by the first thread to swap its
// mark to the number of the cast in which the world's receivers were last stood afresh
// (`light::HDR_WORLD`, `light::LIST_READS`). It stays listed, as the tiles stood on do,
// until the receivers are stood afresh again (`light::carry_at`). So a cast propagates only the
// tiles stood on since the casts before it propagated theirs, and keeps their reads. Then
// `light_damage_cs` takes one thread for each tile listed as read. It damages each tile that no
// cast of the reads' epoch damaged (`reads_epoch`): the tile's word names the cast
// (`light::reads_at`), and what feeds the tile is listed for the passes that cast it
// (`enlist`). It also opens the map's lay to every tile read (`light::ARGS_LAY`; pass/shadow.rs
// `Shadow::begin`).

/// Whether box texel `p` lies where a receiver may read the map, in the disk grown by `pad`
/// texels (`Light::read`).
fn reads_texel(size: u32, p: vec2<i32>, pad: f32) -> bool {
  let half = f32(size) * 0.5;
  let d = vec2<f32>(p) + vec2<f32>(0.5) - vec2<f32>(half);
  let r = eye_f32(EYE_LIGHT + LIGHT_REACH_NDC) * half + pad;
  return d.x * d.x + d.y * d.y <= r * r;
}

/// The way up from a point to the height of the tallest decor, as it shows on the box in tiles
/// across and down, and the number of steps that take it at most a tile apart.
struct Up {
  way: vec2<f32>,
  steps: u32,
}

fn up() -> Up {
  let low = texel_at(vec3<f32>(0.0));
  let top = texel_at(vec3<f32>(0.0, 0.0, f32(TALLEST)));
  let way = (top - low) * (1.0 / f32(LIGHT_TILE));
  return Up(way, u32(ceil(max(abs(way.x), abs(way.y)))));
}

/// The `k`-th step of the way up, whole tiles from the tile stood on, rounded down.
fn up_at(u: Up, k: u32) -> vec2<i32> {
  let f = select(f32(k) / f32(u.steps), 0.0, u.steps == 0u);
  return vec2<i32>(floor(u.way * f));
}

/// Lists tile `t` as read since the world's receivers were stood afresh in the cast numbered
/// `world`. It is listed where it holds a slot a receiver reads and is not listed yet. Its mark
/// is read first and swapped only where it differs, so a tile many receivers read takes one
/// swap.
fn read_tile(world: u32, t: vec2<i32>) {
  let n = light_tiles();
  if (any(t < vec2<i32>(0)) || any(t >= vec2<i32>(i32(n)))) {
    return;
  }
  // A tile whose middle lies outside the disk grown by half the tile's diagonal holds no slot
  // a receiver reads (`light::TILE_READ_PAD`).
  let middle = t * i32(LIGHT_TILE) + vec2<i32>(i32(LIGHT_TILE / 2u));
  if (!reads_texel(light_size(), middle, LIGHT_TILE_READ_PAD)) {
    return;
  }
  let place = u32(t.y) * n + u32(t.x);
  let mark = list_marks_at(LIGHT_LIST_READS) + place;
  if (atomicLoad(&args.marks[mark]) == world || atomicExchange(&args.marks[mark], world) == world) {
    return;
  }
  let e = atomicAdd(&args.cnts[LIGHT_LIST_READS], 1u);
  if (e < n * n) {
    list_put(list_cnt_at(LIGHT_LIST_READS) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
  }
}

@compute @workgroup_size(MISS_LANE_CNT, 1, 1)
fn light_propagate_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  // The thread's place in the stood list, counted past the tiles the casts before propagated
  // (`light::carry_at`), as list.wgsl `list_place` numbers a list walked apart.
  let n = light_tiles();
  let propagated = in_cap(list_at(hdr_word(LIGHT_HDR_CARRY) + LIGHT_RECEIVER_TILES_CNT), n * n);
  let k = propagated + (group.y * MISS_ACROSS + group.x) * MISS_LANE_CNT + lane.x;
  let at = list_cnt_at(LIGHT_LIST_RECEIVERS);
  if (k >= in_cap(list_at(at), n * n)) {
    return;
  }
  let w = list_at(at + 1u + k);
  let s = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let world = hdr_word(LIGHT_HDR_WORLD);
  let u = up();
  // Step `k` lies at least `k - 1` tiles from the tile stood on along the way's longer axis. A
  // step `n` tiles and the taps' span away reads only tiles past the box, so the steps past
  // that, with one spare for rounding, read nothing and are not taken.
  let most = n + u32(LIGHT_READ_LAST - LIGHT_READ_FIRST) + 3u;
  for (var j = 0u; j <= min(u.steps, most); j += 1u) {
    let o = up_at(u, j);
    for (var b = LIGHT_READ_FIRST; b <= LIGHT_READ_LAST; b += 1i) {
      for (var a = LIGHT_READ_FIRST; a <= LIGHT_READ_LAST; a += 1i) {
        read_tile(world, s + o - vec2<i32>(a, b));
      }
    }
  }
}

/// Opens the map's lay to every tile listed as read, and damages each such tile that no cast of
/// the reads' epoch damaged, one thread for each tile.
@compute @workgroup_size(LIGHT_LIST_LANE_CNT, 1, 1)
fn light_damage_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let n = light_tiles();
  let read = in_cap(atomicLoad(&args.cnts[LIGHT_LIST_READS]), n * n);
  if (id.x == 0u) {
    atomicStore(&args.passes[LIGHT_ARGS_LAY + 1u], read);
  }
  if (id.x >= read) {
    return;
  }
  let w = list_at(list_cnt_at(LIGHT_LIST_READS) + 1u + id.x);
  let t = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let at = reads_at() + u32(t.y) * n + u32(t.x);
  if (list_at(at) < reads_epoch()) {
    let cur = cast_number();
    list_put(at, cur);
    enlist(cur, t);
  }
}
