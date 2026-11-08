// pxl-render/light_receiver_tile.wgsl

// How the receivers' passes (light_mark.wgsl, light_mark_draws.wgsl) list the tiles of the box
// a receiver stands on (pxl-engine `light::LIST_RECEIVERS`). The propagation then takes one
// thread for each tile listed (light_propagate.wgsl). Each tile is listed once while it
// stands, by the first thread to swap its mark to the number of the cast in which the world's
// receivers were last stood afresh (`light::HEADER_WORLD`). The world's receivers and the
// drawn ones list alike, so a tile stays listed in every cast after it until the receivers are
// stood afresh again (`light::carry_at`). A mark is read first and swapped only where it
// differs, so a tile that many receivers stand on takes one swap in a cast that stands it
// anew. The list's count and the propagation's rows are the stood list's args, raised as
// list.wgsl `enlist` raises those of a list walked apart.

/// The stood list's args (pxl-engine `light::RECEIVER_TILES_WORDS`): the propagation's dispatch
/// and the count, under the names `enlist` reads them by, then the slide's dispatch. The cast's
/// first stood tile opens the slide's workgroups across, and the host sets its workgroups down
/// where the cast owes a slide.
struct ReceiverArgs {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
  slide: array<atomic<u32>, 3>,
}

@group(0) @binding(22) var<storage, read_write> args: ReceiverArgs;
/// The light's args as words, whose marks lie past `light::ARGS_WORDS` (`light::args_words`).
@group(0) @binding(21) var<storage, read_write> light_args: array<atomic<u32>>;

/// Where the marks begin among the light's args, past the passes' words and the lists' counts.
const MARKS_FIRST: u32 = LIGHT_ARGS_PASSES + LIGHT_LISTS;

/// What the one thread that lists the cast's first stood tile does. It opens the slide's
/// workgroups across, so a cast that owes a slide runs it once a receiver stands. Where the
/// slide last ran before the last cast that owed one, that cast stood no receiver and so ran no
/// slide, and the thread begins the reads' epoch anew (`light::READS_FROM`).
fn receiver_tiles_first() {
  let n = light_tiles();
  atomicStore(&args.slide[0], workgroups(n, n, SQUARE_SIDE));
  let at = header_word(LIGHT_HEADER_SLIDE);
  if (list_at(at + LIGHT_SLIDE_RAN) < header_word(LIGHT_HEADER_SLIDE_CAST)) {
    list_put(at + LIGHT_READS_FROM, cast_number());
  }
}

/// Lists tile `t` as stood on. Past the list's room it is not listed.
fn receiver_tiles_put(t: vec2<i32>) {
  let n = light_tiles();
  let cap = n * n;
  let e = enlist(cap);
  if (e >= cap) {
    return;
  }
  list_put(list_cnt_at(LIGHT_LIST_RECEIVERS) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
  if (e == 0u) {
    receiver_tiles_first();
  }
}

/// Stands a receiver on tile `t`. The tile is listed where no receiver stood on it since the
/// world's receivers were last stood afresh. A tile past the box is not listed.
fn stand(t: vec2<i32>) {
  let w = tile_place(t);
  if (w == NO_TILE) {
    return;
  }
  let world = header_word(LIGHT_HEADER_WORLD);
  let mark = MARKS_FIRST + list_marks_at(LIGHT_LIST_RECEIVERS) + w;
  if (atomicLoad(&light_args[mark]) != world && atomicExchange(&light_args[mark], world) != world) {
    receiver_tiles_put(t);
  }
}

/// Stands a receiver on the tiles from `first` to `last`, held to the box.
fn stand_box(first: vec2<i32>, last: vec2<i32>) {
  let n = i32(light_tiles());
  let lo = max(first, vec2<i32>(0));
  let hi = min(last, vec2<i32>(n - 1i));
  for (var j = lo.y; j <= hi.y; j += 1i) {
    for (var i = lo.x; i <= hi.x; i += 1i) {
      stand(vec2<i32>(i, j));
    }
  }
}
