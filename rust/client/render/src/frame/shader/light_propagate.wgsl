// pxl-render/light_propagate.wgsl

// Which tiles of the box a receiver reads in this cast, a thread a tile a receiver stands on
// (`light::LISTED_STOOD`; light_mark.wgsl, light_mark_draws.wgsl), after every receiver stood
// and the slide (light_slide.wgsl), before the beam: those in the disk it reads
// (`Light::read`) within a tile and its taps of a step up the way up from it (`up`, `up_at`:
// a point that far up lies that many tiles over or one more, its taps a tile either side), as
// decor and the actors stand on what the eye meets and are no taller. Each is listed read once
// a cast, by the first thread to swap its mark (`light::LISTED_READ`), and the map's lay opened
// to it (`light::ARGS_LAY`; gpu/shadow.rs `Shadow::begin`). One no cast of the reads' epoch
// read (`reads_epoch`) is the cast's to cast: its word names the cast (`light::reads_at`),
// where it lies within the epoch's budget (`within_budget`); what feeds it is listed for the passes that cast it (`enlist`). A tile's
// read words are the thread's alone that listed it read.

/// The workgroups of the passes that cast tiles; the lists' counts and marks
/// (`light::ARGS_WORDS`). A claim reads and writes the passes', the counts and the marks at once.
struct Args {
  passes: array<atomic<u32>, LIGHT_ARGS_PASSES>,
  counts: array<atomic<u32>, LIGHT_LISTS>,
  marks: array<atomic<u32>>,
}

@group(0) @binding(21) var<storage, read_write> args: Args;

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

/// Pass `w`'s rows raised to hold entry `e` of a list of `room` places, its threads one of
/// `across` a side of the entry's tile each (pxl-engine `light::LIST_ACROSS`).
fn opened(w: u32, e: u32, room: u32, across: u32) {
  let per = LIGHT_LIST_ACROSS * LIGHT_LISTED_LANES;
  let square = across * across;
  atomicMax(&args.passes[w + 1u], workgroups((e + 1u) * square, room * square, per));
}

/// The places of list `which`, `across` a row, whose reach `lo`..`hi` about the tile
/// they lie on, less `off`, holds claimed tile `t`, listed where no claim of the cast
/// listed them yet (`light::listed_marks_at`), and the rows of the passes that take
/// them opened (`opened`).
fn enlist_in(current: u32, t: vec2<i32>, which: u32, across: i32, lo: vec2<i32>, hi: vec2<i32>, off: vec2<i32>) {
  let marks = listed_marks_at(which);
  let at = listed_at(which);
  let room = listed_room(which);
  let first = t - hi - off;
  let last = t - lo - off;
  for (var j = first.y; j <= last.y; j += 1i) {
    for (var i = first.x; i <= last.x; i += 1i) {
      if (i >= 0i && j >= 0i && i < across && j < across
        && atomicExchange(&args.marks[marks + u32(j * across + i)], current) != current) {
        let e = atomicAdd(&args.counts[which], 1u);
        if (e >= room) {
          continue;
        }
        list_put(at + 1u + e, (u32(j) << 16u) | u32(i));
        if (which == LIGHT_LISTED_CLAIMED) {
          opened(LIGHT_ARGS_SLOT, e, room, LIGHT_TILE);
        } else if (which == LIGHT_LISTED_FED) {
          let s = LIGHT_TILE / LIGHT_SAMPLE;
          opened(LIGHT_ARGS_CELLS, e, room, 1u);
          opened(LIGHT_ARGS_SAMPLE, e, room, s);
          opened(LIGHT_ARGS_SAMPLE_BOX, e, room, s);
        } else {
          opened(LIGHT_ARGS_BEAM, e, room, 1u);
        }
      }
    }
  }
}

/// What feeds claimed tile `t` listed at each level: a slot its own tile; a cell a tile either
/// way; a beam ray the cells whose
/// window of `2 + 2 · BEAM_WINDOW` holds it, its number less the tile beam ray 0 lies on
/// (`light_on`).
fn enlist(current: u32, t: vec2<i32>) {
  let tiles = i32(light_tiles());
  enlist_in(current, t, LIGHT_LISTED_CLAIMED, tiles, vec2<i32>(0), vec2<i32>(0), vec2<i32>(0));
  enlist_in(current, t, LIGHT_LISTED_FED, tiles, vec2<i32>(-1), vec2<i32>(1), vec2<i32>(0));
  let w = i32(LIGHT_BEAM_WINDOW);
  enlist_in(
    current, t, LIGHT_LISTED_CORNERS, i32(beam_across()),
    vec2<i32>(-w - 2i), vec2<i32>(w + 1i), light_on(LEVEL_CORNER, vec2<u32>(0u)),
  );
}

/// Whether tile `t` lies within what the epoch's casts so far may claim (`light::CLAIM_BUDGET`):
/// the disk about the box's middle holding that many tiles for each cast of the epoch, this
/// one's included.
fn within_budget(t: vec2<i32>, current: u32, epoch: u32) -> bool {
  let half = f32(light_tiles()) * 0.5;
  let d = vec2<f32>(t) + vec2<f32>(0.5) - vec2<f32>(half);
  let casts = f32(current - epoch + 1u);
  return 3.14159265 * dot(d, d) <= casts * f32(LIGHT_CLAIM_BUDGET);
}

/// Tile `t` read in the cast numbered `current`, its reads' epoch `epoch`: listed read where it
/// holds a slot a receiver reads and is not listed yet, and claimed where no cast of the epoch
/// read it and it lies within the budget.
fn read_tile(current: u32, epoch: u32, t: vec2<i32>) {
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
  if (atomicExchange(&args.marks[listed_marks_at(LIGHT_LISTED_READ) + place], current) == current) {
    return;
  }
  let room = n * n;
  let e = atomicAdd(&args.counts[LIGHT_LISTED_READ], 1u);
  if (e < room) {
    list_put(listed_at(LIGHT_LISTED_READ) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
    atomicMax(&args.passes[LIGHT_ARGS_LAY + 1u], in_room(e + 1u, room));
  }
  // Read, read first in the epoch and within its budget: the cast's to cast. One past the
  // budget is left to a later cast, its word as it was.
  let w = reads_at() + place;
  if (list_at(w) < epoch && within_budget(t, current, epoch)) {
    list_put(w, current);
    enlist(current, t);
  }
}

@compute @workgroup_size(MISS_LANES, 1, 1)
fn light_propagate_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  // Its place in the stood list, as list.wgsl's `list_place` numbers a list walked apart.
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  let n = light_tiles();
  let at = listed_at(LIGHT_LISTED_STOOD);
  if (k >= in_room(list_at(at), n * n)) {
    return;
  }
  let w = list_at(at + 1u + k);
  let s = vec2<i32>(i32(w & 0xffffu), i32(w >> 16u));
  let current = cast_number();
  let epoch = reads_epoch();
  let u = up();
  // Step `k` lies `k - 1` tiles or more from the tile stood on along the way's longer axis,
  // and a step `n` tiles and its taps' span off reads only tiles past the box: the steps past
  // that, one spare for rounding, read nothing.
  let most = n + u32(LIGHT_READ_LAST - LIGHT_READ_FIRST) + 3u;
  for (var j = 0u; j <= min(u.steps, most); j += 1u) {
    let o = up_at(u, j);
    for (var b = LIGHT_READ_FIRST; b <= LIGHT_READ_LAST; b += 1i) {
      for (var a = LIGHT_READ_FIRST; a <= LIGHT_READ_LAST; a += 1i) {
        read_tile(current, epoch, s + o - vec2<i32>(a, b));
      }
    }
  }
}
