// pxl-render/light_stood.wgsl

// The tiles of the box a receiver stands on, as the receivers' passes list them (pxl-engine
// `light::LISTED_STOOD`; light_mark.wgsl, light_mark_draws.wgsl), for the propagation, a
// thread a tile listed (light_propagate.wgsl). Each tile is listed once while it stands: by
// the first thread to swap its mark to the number of the cast the world's receivers were last
// stood afresh in (`light::HEAD_WORLD`), the world's receivers' and the drawn ones' alike, so
// it stays listed in every cast after it until they are stood afresh again (`light::kept_at`).
// A mark read first is swapped only where it differs, so a tile many receivers stand on takes
// one swap a cast that stands it anew. The list's count and the propagation's rows are the
// stood list's args', raised as list.wgsl `enlist` raises a list walked apart's.

/// The stood list's args (pxl-engine `light::STOOD_WORDS`): the propagation's dispatch and the
/// count, as `enlist` names them; then the slide's dispatch, its workgroups across opened by
/// the cast's first stood tile, down the host's where the cast owes a slide.
struct Stood {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
  slide: array<atomic<u32>, 3>,
}

@group(0) @binding(22) var<storage, read_write> args: Stood;
/// The light's args as words, whose marks lie past `light::ARGS_WORDS` (`light::args_words`).
@group(0) @binding(21) var<storage, read_write> light_args: array<atomic<u32>>;

/// The first of the light's args' marks: past the passes' words and the lists' counts.
const MARKS_FIRST: u32 = LIGHT_ARGS_PASSES + LIGHT_LISTS;

/// What the cast's first stood tile does, the one thread that lists it: the slide's workgroups
/// across opened, so a cast that owes a slide runs it once a receiver stands; and the reads'
/// epoch begun anew where the slide last ran before the last cast that owed one, which stood
/// none and so ran none (`light::READS_FROM`).
fn stood_first() {
  let n = light_tiles();
  atomicStore(&args.slide[0], workgroups(n, n, SQUARE_SIDE));
  let at = head_word(LIGHT_HEAD_SLIDE);
  if (list_at(at + LIGHT_SLIDE_RAN) < head_word(LIGHT_HEAD_SLIDE_OWED)) {
    list_put(at + LIGHT_READS_FROM, cast_number());
  }
}

/// Tile `t` listed stood on, none past the list's room.
fn stood_put(t: vec2<i32>) {
  let n = light_tiles();
  let room = n * n;
  let e = enlist(room);
  if (e >= room) {
    return;
  }
  list_put(listed_at(LIGHT_LISTED_STOOD) + 1u + e, (u32(t.y) << 16u) | u32(t.x));
  if (e == 0u) {
    stood_first();
  }
}

/// Tile `t` stood on by a receiver, listed where none stood on it since the world's receivers
/// were last stood afresh; none past the box.
fn stand(t: vec2<i32>) {
  let w = tile_place(t);
  if (w == NO_TILE) {
    return;
  }
  let world = head_word(LIGHT_HEAD_WORLD);
  let mark = MARKS_FIRST + listed_marks_at(LIGHT_LISTED_STOOD) + w;
  if (atomicLoad(&light_args[mark]) != world && atomicExchange(&light_args[mark], world) != world) {
    stood_put(t);
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
