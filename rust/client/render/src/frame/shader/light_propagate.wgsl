// pxl-render/light_propagate.wgsl

// Which tiles of the box a receiver reads, a thread a tile a receiver stands on
// (`light::LISTED_STOOD`; light_mark.wgsl, light_mark_draws.wgsl), after every receiver stood
// and the slide (light_slide.wgsl), before the beam: those in the disk it reads
// (`Light::read`) within a tile and its taps of a step up the way up from it (`up`, `up_at`:
// a point that far up lies that many tiles over or one more, its taps a tile either side), as
// decor and the actors stand on what the eye meets and are no taller. A tile is listed read by
// the first thread to swap its mark to the number of the cast the world's receivers were last
// stood afresh in (`light::HEAD_WORLD`, `light::LISTED_READ`), and stays listed, as the tiles
// stood on do, until they are stood afresh again (`light::kept_at`): so a cast propagates only
// the tiles stood on since the casts before it propagated theirs, whose reads it keeps.
// Then, a thread a tile listed read, each one no cast of the reads' epoch claimed
// (`reads_epoch`), or owed (`light::READ_OWED`), is wanted (`light::LISTED_WANTED`), counted by
// its ring (`light::rings_at`), and the map's lay opened to every tile read (`light::ARGS_LAY`;
// gpu/shadow.rs `Shadow::begin`) (`light_want_cs`); the claim takes the wanted nearest first,
// at most `light::CLAIM_BUDGET` (`light_claim_cs`, `light::claims`), the one way a tile is
// claimed: its word names the cast (`light::reads_at`), it takes a place for its slots' parked
// rays (`light::places_at`), and what feeds it is listed for the passes that cast it
// (`enlist`). A tile's read words are the thread's alone that listed it read, then the one
// that claims it.

/// Whether box texel `p` lies where a receiver may read it, grown by `pad` texels
/// (`Light::read`).
fn reads_texel(size: u32, p: vec2<i32>, pad: f32) -> bool {
  let half = f32(size) * 0.5;
  let d = vec2<f32>(p) + vec2<f32>(0.5) - vec2<f32>(half);
  let r = eye_f32(LIGHT + LIGHT_REACH_NDC) * half + pad;
  return d.x * d.x + d.y * d.y <= r * r;
}

/// The way up decor's tallest on the box, in tiles across and down, and the steps it
/// takes a tile apart at most.
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

/// The claim's dispatch over the tiles wanted (`light::CLAIM_DISPATCH`), a buffer of its own,
/// its rows raised here; light_claim.wgsl reads its count from it and does not bind it.
@group(0) @binding(25) var<storage, read_write> claims: array<atomic<u32>, 3>;

/// Tile `t` wanted: listed for the claim, the claim's rows opened to it, and counted by its
/// ring.
fn want(t: vec2<i32>) {
  let n = light_tiles();
  let room = n * n;
  let e = atomicAdd(&args.counts[LIGHT_LISTED_WANTED], 1u);
  if (e >= room) {
    return;
  }
  list_put(listed_at(LIGHT_LISTED_WANTED) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
  atomicMax(&claims[1], workgroups(e + 1u, room, LIGHT_LIST_ACROSS * LIGHT_LISTED_LANES));
  atomicAdd(&args.marks[head_word(LIGHT_HEAD_RINGS) + ring(t)], 1u);
}

/// Tile `t` read since the world's receivers were stood afresh in the cast numbered `world`:
/// listed read where it holds a slot a receiver reads and is not listed yet, its mark read
/// first and swapped only where it differs.
fn read_tile(world: u32, t: vec2<i32>) {
  let n = light_tiles();
  if (any(t < vec2<i32>(0)) || any(t >= vec2<i32>(i32(n)))) {
    return;
  }
  // A tile past every slot a receiver reads holds none of its slots.
  let middle = t * i32(LIGHT_TILE) + vec2<i32>(i32(LIGHT_TILE / 2u));
  if (!reads_texel(light_size(), middle, LIGHT_TILE_READ_PAD)) {
    return;
  }
  let place = u32(t.y) * n + u32(t.x);
  let mark = listed_marks_at(LIGHT_LISTED_READ) + place;
  if (atomicLoad(&args.marks[mark]) == world || atomicExchange(&args.marks[mark], world) == world) {
    return;
  }
  let e = atomicAdd(&args.counts[LIGHT_LISTED_READ], 1u);
  if (e < n * n) {
    list_put(listed_at(LIGHT_LISTED_READ) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
  }
}

@compute @workgroup_size(MISS_LANES, 1, 1)
fn light_propagate_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  // Its place in the stood list past the tiles the casts before propagated (`light::kept_at`),
  // as list.wgsl's `list_place` numbers a list walked apart.
  let n = light_tiles();
  let propagated = in_room(list_at(head_word(LIGHT_HEAD_KEPT) + LIGHT_STOOD_CNT), n * n);
  let k = propagated + (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  let at = listed_at(LIGHT_LISTED_STOOD);
  if (k >= in_room(list_at(at), n * n)) {
    return;
  }
  let w = list_at(at + 1u + k);
  let s = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let world = head_word(LIGHT_HEAD_WORLD);
  let u = up();
  // Step `k` lies `k - 1` tiles or more from the tile stood on along the way's longer axis,
  // and a step `n` tiles and its taps' span off reads only tiles past the box: the steps past
  // that, one spare for rounding, read nothing.
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

/// Each tile listed read, a thread each over every place of the box, after the propagation: the
/// map's lay opened to them all, and each wanted where no cast of the reads' epoch claimed it
/// or it is owed (`want`), so a tile a cast left wanted, past its budget or owed, is wanted
/// again by the next however long ago it was listed.
@compute @workgroup_size(LIGHT_LISTED_LANES, 1, 1)
fn light_want_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let n = light_tiles();
  let read = in_room(atomicLoad(&args.counts[LIGHT_LISTED_READ]), n * n);
  if (id.x == 0u) {
    atomicStore(&args.passes[LIGHT_ARGS_LAY + 1u], read);
  }
  if (id.x >= read) {
    return;
  }
  let w = list_at(listed_at(LIGHT_LISTED_READ) + 1u + id.x);
  let t = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let word = list_at(reads_at() + u32(t.y) * n + u32(t.x));
  if ((word & ~LIGHT_READ_OWED) < reads_epoch() || (word & LIGHT_READ_OWED) != 0u) {
    want(t);
  }
}
