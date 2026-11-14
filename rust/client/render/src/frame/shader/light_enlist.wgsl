// pxl-render/frame/shader/light_enlist.wgsl

// How the propagation (light_propagate.wgsl) writes the light's args and lists. A damaged tile
// lists what feeds it at each level of the lattice, and raises the rows of the passes that
// take those entries. The args hold the passes' workgroups, the lists' counts and the lists'
// marks.

/// The workgroups of the passes that cast tiles, then the lists' counts and marks
/// (`light::ARGS_WORDS`). The damage pass reads and writes all three together.
struct Dispatch {
  passes: array<atomic<u32>, LIGHT_ARGS_PASSES>,
  cnts: array<atomic<u32>, LIGHT_LIST_CNT>,
  marks: array<atomic<u32>>,
}

@group(0) @binding(21) var<storage, read_write> args: Dispatch;

/// Raises the rows of pass `w` to hold entry `e` of a list of `cap` places. The pass takes each
/// entry with a square of `across` threads a side, and lays `light::LIST_ACROSS` workgroups in
/// a row (pxl-engine).
fn opened(w: u32, e: u32, cap: u32, across: u32) {
  let per = LIGHT_LIST_ACROSS * LIGHT_LIST_LANE_CNT;
  let square = across * across;
  atomicMax(&args.passes[w + 1u], workgroup_cnt((e + 1u) * square, cap * square, per));
}

/// Lists the places of list `which`, `across` a row, that feed damaged tile `t`. Place `p` lies
/// on tile `p + off`, and it feeds `t` where its reach `lo`..`hi` about that tile holds `t`. A
/// place is listed only where no damaged tile of this cast listed it yet (`light::list_marks_at`),
/// and the rows of the passes that take it are raised (`opened`).
fn enlist_in(cur: u32, t: vec2<i32>, which: u32, across: i32, lo: vec2<i32>, hi: vec2<i32>, off: vec2<i32>) {
  let marks = list_marks_at(which);
  let at = list_cnt_at(which);
  let cap = list_cap(which);
  let first = t - hi - off;
  let last = t - lo - off;
  for (var j = first.y; j <= last.y; j += 1i) {
    for (var i = first.x; i <= last.x; i += 1i) {
      if (i >= 0i && j >= 0i && i < across && j < across
        && atomicExchange(&args.marks[marks + u32(j * across + i)], cur) != cur) {
        let e = atomicAdd(&args.cnts[which], 1u);
        if (e >= cap) {
          continue;
        }
        list_put(at + 1u + e, (u32(j) << 16u) | u32(i));
        if (which == LIGHT_LIST_DAMAGE) {
          opened(LIGHT_ARGS_SLOT, e, cap, LIGHT_TILE);
        } else if (which == LIGHT_LIST_SOURCES) {
          let s = LIGHT_TILE / LIGHT_SAMPLE;
          opened(LIGHT_ARGS_CELLS, e, cap, 1u);
          opened(LIGHT_ARGS_SAMPLE_HIT, e, cap, s);
        }
      }
    }
  }
}

/// Lists what feeds damaged tile `t` at each level. A tile's slots feed only that tile. A
/// tile's cells and samples feed it and the tiles one either way. A beam ray feeds the cells
/// whose window of `2 + 2 · BEAM_WINDOW` beam rays holds it, and the beam rays are numbered
/// from the tile that beam ray 0 lies on (`light_on`).
fn enlist(cur: u32, t: vec2<i32>) {
  let tiles = i32(light_tiles());
  enlist_in(cur, t, LIGHT_LIST_DAMAGE, tiles, vec2<i32>(0), vec2<i32>(0), vec2<i32>(0));
  enlist_in(cur, t, LIGHT_LIST_SOURCES, tiles, vec2<i32>(-1), vec2<i32>(1), vec2<i32>(0));
  let w = i32(LIGHT_BEAM_WINDOW);
  enlist_in(
    cur, t, LIGHT_LIST_CORNERS, i32(beam_across()),
    vec2<i32>(-w - 2i), vec2<i32>(w + 1i), light_on(LEVEL_CORNER, vec2<u32>(0u)),
  );
}
