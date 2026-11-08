// pxl-render/light_receiver_tiles.wgsl

// The tiles of the box a receiver stands on, as the receivers' passes list them (pxl-engine
// `light::LIST_RECEIVERS`; light_mark.wgsl, light_mark_draws.wgsl), for the propagation, a
// thread a tile listed (light_propagate.wgsl). Each tile is listed once while it stands: by
// the first thread to swap its mark to the number of the cast the world's receivers were last
// stood afresh in (`light::HEADER_WORLD`), the world's receivers' and the drawn ones' alike, so
// it stays listed in every cast after it until they are stood afresh again (`light::carry_at`).
// A mark read first is swapped only where it differs, so a tile many receivers stand on takes
// one swap a cast that stands it anew. The list's count and the propagation's rows are the
// stood list's args', raised as list.wgsl `enlist` raises a list walked apart's.

/// The stood list's args (pxl-engine `light::RECEIVER_TILES_WORDS`): the propagation's dispatch and the
/// count, as `enlist` names them; then the slide's dispatch, its workgroups across opened by
/// the cast's first stood tile, down the host's where the cast owes a slide.
struct ReceiverArgs {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
  slide: array<atomic<u32>, 3>,
}

@group(0) @binding(22) var<storage, read_write> args: ReceiverArgs;
/// The light's args as words, whose marks lie past `light::ARGS_WORDS` (`light::args_words`).
@group(0) @binding(21) var<storage, read_write> light_args: array<atomic<u32>>;

/// The first of the light's args' marks: past the passes' words and the lists' counts.
const MARKS_FIRST: u32 = LIGHT_ARGS_PASSES + LIGHT_LISTS;

/// What the cast's first stood tile does, the one thread that lists it: the slide's workgroups
/// across opened, so a cast that owes a slide runs it once a receiver stands; and the reads'
/// epoch begun anew where the slide last ran before the last cast that owed one, which stood
/// none and so ran none (`light::READS_FROM`).
fn receiver_tiles_first() {
  let n = light_tiles();
  atomicStore(&args.slide[0], workgroups(n, n, SQUARE_SIDE));
  let at = header_word(LIGHT_HEADER_SLIDE);
  if (list_at(at + LIGHT_SLIDE_RAN) < header_word(LIGHT_HEADER_SLIDE_CAST)) {
    list_put(at + LIGHT_READS_FROM, cast_number());
  }
}

/// Tile `t` listed stood on, none past the list's room.
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

/// Tile `t` stood on by a receiver, listed where none stood on it since the world's receivers
/// were last stood afresh; none past the box.
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

/// The tiles from `first` to `last` stood on by a receiver, kept to the box.
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
