// pxl-render/light_propagate.wgsl

// Which tiles of the box a receiver reads in this cast, a thread a tile, after every
// receiver stood (light_mark.wgsl, light_mark_draws.wgsl), before the beam: one in the
// disk it reads (`Light::read`), within a tile and its taps of a step up the way up from a
// tile one stands on (`Light::up`, `Light::up_at`: a point that far up lies that many
// tiles over or one more, its taps a tile either side), as decor and the actors stand on
// what the eye meets and are no taller. One read that no cast of the epoch read is the
// cast's to cast: its word names the cast (`light::reads_at`), and its square's
// (`light::coarse_at`). A tile's words are its own thread's alone. One claimed where the
// cast begins its epoch sets the passes that cast tiles whole (`light::ARGS_WORDS`), each
// claim writing the same words; else what feeds it is listed for them (`enlist`).

/// The workgroups of the passes that cast tiles (`light::ARGS_WORDS`).
@group(0) @binding(21) var<storage, read_write> args: array<atomic<u32>>;

fn args_at(i: u32) -> u32 {
  if (i < arrayLength(&args)) {
    return atomicLoad(&args[i]);
  }
  return 0u;
}

fn args_put(i: u32, v: u32) {
  if (i < arrayLength(&args)) {
    atomicStore(&args[i], v);
  }
}

fn args_most(i: u32, v: u32) {
  if (i < arrayLength(&args)) {
    atomicMax(&args[i], v);
  }
}

/// Whether box texel `p` lies where a receiver may read it, grown by `pad` texels
/// (`Light::read`).
fn reads_texel(size: u32, p: vec2<i32>, pad: f32) -> bool {
  let half = f32(size) / 2.0;
  let d = vec2<f32>(p) + vec2<f32>(0.5) - vec2<f32>(half);
  let r = eye_f32(LIGHT + LIGHT_REACH_NDC) * half + pad;
  return d.x * d.x + d.y * d.y <= r * r;
}

/// The way up decor's tallest on the box, in tiles across and down, and the steps it
/// takes a tile apart at most (`Light::up`).
struct Up {
  way: vec2<f32>,
  steps: u32,
}

fn up() -> Up {
  let low = texel_at(vec3<f32>(0.0));
  let top = texel_at(vec3<f32>(0.0, 0.0, f32(TALLEST)));
  let way = (top - low) / f32(LIGHT_TILE);
  return Up(way, u32(ceil(max(abs(way.x), abs(way.y)))));
}

/// The `k`-th step of the way up, whole tiles from the tile stood on, rounded down
/// (`Light::up_at`).
fn up_at(u: Up, k: u32) -> vec2<i32> {
  let f = select(f32(k) / f32(u.steps), 0.0, u.steps == 0u);
  return vec2<i32>(floor(u.way * f));
}

/// Pass `w`'s rows raised to hold entry `e` of the list it takes, its threads one of
/// `across` a side of the entry's tile each (pxl-engine `light::LIST_ACROSS`).
fn opened(w: u32, e: u32, across: u32) {
  let per = LIGHT_LIST_ACROSS * LIGHT_LISTED_LANES;
  args_most(w + 1u, ((e + 1u) * across * across + per - 1u) / per);
}

/// The places of list `which`, `across` a row, whose reach `lo`..`hi` about the tile
/// they lie on, less `off`, holds claimed tile `t`, listed where no claim of the cast
/// listed them yet (`light::listed_marks_at`), and the rows of the passes that take
/// them opened (`opened`).
fn enlist_in(current: u32, t: vec2<i32>, which: u32, across: i32, lo: vec2<i32>, hi: vec2<i32>, off: vec2<i32>) {
  let marks = listed_marks_at(which);
  let at = listed_at(which);
  let first = t - hi - off;
  let last = t - lo - off;
  for (var j = first.y; j <= last.y; j += 1i) {
    for (var i = first.x; i <= last.x; i += 1i) {
      if (i >= 0i && j >= 0i && i < across && j < across
        && list_raise(marks + u32(j * across + i), current) != current) {
        let e = list_add(at, 1u);
        list_put(at + 1u + e, (u32(j) << 16u) | u32(i));
        if (which == LIGHT_LISTED_CLAIMED) {
          opened(LIGHT_ARGS_SLOT, e, LIGHT_TILE);
        } else if (which == LIGHT_LISTED_FED) {
          let s = LIGHT_TILE / LIGHT_SAMPLE;
          opened(LIGHT_ARGS_CELLS, e, 1u);
          opened(LIGHT_ARGS_SAMPLE, e, s);
          opened(LIGHT_ARGS_SAMPLE_BOX, e, s);
        } else {
          opened(LIGHT_ARGS_BEAM, e, 1u);
        }
      }
    }
  }
}

/// What feeds claimed tile `t` listed at each level (light_list.wgsl `light_needs`, its
/// rule the other way): a slot its own tile; a cell a tile either way; a beam ray the cells whose
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

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_propagate_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = light_size();
  let n = light_tiles();
  if (id.x >= n || id.y >= n) {
    return;
  }
  // A tile past every slot a receiver reads holds none of its slots.
  let middle = vec2<i32>(id.xy * LIGHT_TILE + vec2<u32>(LIGHT_TILE / 2u));
  if (!reads_texel(size, middle, LIGHT_TILE_READ_PAD)) {
    return;
  }
  let current = cast_number();
  let epoch = light_epoch();
  let u = up();
  let t = vec2<i32>(id.xy);
  var is_read = false;
  for (var k = 0u; k <= u.steps && !is_read; k += 1u) {
    let o = up_at(u, k);
    for (var b = LIGHT_READ_FIRST; b <= LIGHT_READ_LAST && !is_read; b += 1i) {
      for (var a = LIGHT_READ_FIRST; a <= LIGHT_READ_LAST && !is_read; a += 1i) {
        is_read = stood(current, t - o + vec2<i32>(a, b));
      }
    }
  }
  // Read, and read first in the epoch: the cast's to cast.
  let w = reads_at() + id.y * n + id.x;
  if (is_read && list_at(w) < epoch) {
    list_put(w, current);
    list_put(coarse_square(id.xy), current);
    if (current == epoch) {
      for (var k = 0u; k < LIGHT_ARGS_WHOLE; k += 1u) {
        args_put(k, args_at(LIGHT_ARGS_WHOLE + k));
      }
    } else {
      enlist(current, t);
    }
  }
}
