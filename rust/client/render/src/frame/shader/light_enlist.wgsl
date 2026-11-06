// pxl-render/light_enlist.wgsl

// The light's args as the propagation and the claim write them (light_propagate.wgsl,
// light_claim.wgsl): the passes' workgroups, the lists' counts and marks; what feeds a claimed
// tile listed at each level, the rows of the passes that take it opened; and the ring a tile
// lies in.

/// The workgroups of the passes that cast tiles; the lists' counts and marks
/// (`light::ARGS_WORDS`). A claim reads and writes the passes', the counts and the marks at once.
struct Args {
  passes: array<atomic<u32>, LIGHT_ARGS_PASSES>,
  counts: array<atomic<u32>, LIGHT_LISTS>,
  marks: array<atomic<u32>>,
}

@group(0) @binding(21) var<storage, read_write> args: Args;

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
          opened(LIGHT_ARGS_SAMPLE_BOX, e, room, s);
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

/// The ring box tile `t` lies in about the box's middle, whole tiles out (`light::ring`).
fn ring(t: vec2<i32>) -> u32 {
  let half = f32(light_tiles()) * 0.5;
  let d = vec2<f32>(t) + vec2<f32>(0.5) - vec2<f32>(half);
  return u32(sqrt(d.x * d.x + d.y * d.y));
}
