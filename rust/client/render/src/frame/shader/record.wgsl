// pxl-render/record.wgsl

// What draws the node cache's entries (pxl-engine record.rs): an entry's node as a walk from an
// origin holds it, whether that node is the one the eye's walk or a light's ray stops on, and
// the entry of a node a stop draws. Over the node cache as the module binds it, whose words it
// reads through `node_cache_word`. Its writers and their order are record.rs's.

/// None.
const NO_ENTRY: u32 = 0xffffffffu;

/// The frame's number (`node_cache::FRAME`).
fn record_frame() -> u32 {
  return node_cache_word(FRAME);
}

/// A node as a walk holds it, its roots world-absolute (pxl-engine `cast::Node`, its roots
/// added to the origin's).
struct Node {
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
}

/// The root holding the origin `o` on each axis (`cast::roots_of`).
fn origin_roots(o: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(o.x >> 31u, 0i, i32(u32(o.z) + Z_START) >> 31u);
}

/// The node of `zoom` the world names `c` as a walk from the origin `o` holds it
/// (pxl-engine `cast::Node::of_idx`): its corner off the index's low bits, and on each axis the
/// root, the origin's or one either side, whose copy of it has its middle nearer the origin
/// than half a root.
fn node_of_idx(o: vec3<i32>, zoom: u32, c: vec3<i32>) -> Node {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let y_start = select(1u << ((zoom - 1u) & 31u), 0u, zoom == 0u);
  let z_start = select(1u << ((zoom - SHARD_ZOOM - 1u) & 31u), 0u, zoom <= SHARD_ZOOM);
  let low = select((1u << (zoom & 31u)) - 1u, 0xffffffffu, zoom >= FINEST_ZOOM);
  let n = (vec3<u32>(c) + vec3<u32>(0u, y_start, z_start)) & vec3<u32>(low);
  let corner = select(n << vec3<u32>((FINEST_ZOOM - zoom) & 31u), vec3<u32>(0u), zoom == 0u);
  let mid = corner + vec3<u32>(under(zoom) >> 1u);
  let up = mid >= og;
  let d = select(og - mid, mid - og, up);
  let half = vec3<u32>(1u << 31u);
  let root = select(
    select(vec3<i32>(0i), vec3<i32>(1i), d > half),
    select(vec3<i32>(0i), vec3<i32>(-1i), d >= half),
    up,
  );
  return Node(zoom, corner, root + origin_roots(o));
}

/// The node of slot `slot`'s entry as a walk from the origin `o` holds it.
fn entry_kept(slot: u32, o: vec3<i32>) -> Node {
  let n = entry_at(slot) + W_NODE;
  let c = vec3<i32>(i32(node_cache_word(n + 1u)), i32(node_cache_word(n + 2u)), i32(node_cache_word(n + 3u)));
  return node_of_idx(o, node_cache_word(n), c);
}

/// The kind of slot `slot`'s entry where it holds a node, a claim's or a free slot's
/// `KIND_SPLIT`, which no draw takes (pxl-engine `node_cache`).
fn entry_kind(slot: u32) -> u32 {
  let key = node_cache_word(key_at(slot));
  if (key == 0u || key == CLAIMED || (key & CLAIM_MASK) == CLAIM_FAR) {
    return KIND_SPLIT;
  }
  return node_cache_word(entry_at(slot) + W_KIND) & KIND_MASK;
}

/// The slot of the entry that draws the node `c` of `zoom` a ray stopped on, its answer the
/// asset `asset` or the lack: of the entries of its bucket that hold it and answer so, the one
/// that draws already (a face set), else the first; `NO_ENTRY` where none does (pxl-engine
/// `record`). No pass that looks writes a key, so every look of a pass takes one entry.
fn find(zoom: u32, c: vec3<i32>, asset: vec2<u32>, lacks: bool) -> u32 {
  if (!keeps(zoom, c)) {
    return NO_ENTRY;
  }
  let first = bucket_of(zoom, c);
  let kind = select(KIND_CELL, KIND_LACKS, lacks);
  var found = NO_ENTRY;
  for (var k = 0u; k < BUCKET; k += 1u) {
    let slot = first + k;
    let e = entry_at(slot);
    let n = e + W_NODE;
    if (entry_kind(slot) != kind || node_cache_word(n) != zoom
      || any(vec3<u32>(node_cache_word(n + 1u), node_cache_word(n + 2u), node_cache_word(n + 3u)) != vec3<u32>(c))) {
      continue;
    }
    if (!lacks && (node_cache_word(e + W_ASSET) != asset.x || node_cache_word(e + W_ASSET + 1u) != asset.y)) {
      continue;
    }
    if ((node_cache_word(state_at(slot)) & RECORD_FACES) != 0u) {
      return slot;
    }
    if (found == NO_ENTRY) {
      found = slot;
    }
  }
  return found;
}

/// The node's planes in units from the origin `o` (pxl-engine cast.rs `bounds`).
struct Planes {
  lo: vec3<f32>,
  hi: vec3<f32>,
}

fn kept_planes(k: Node, o: vec3<i32>) -> Planes {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let roots = k.roots - origin_roots(o);
  return Planes(units(k.corner, og, roots), far_of(k.corner, under(k.zoom), og, roots));
}

/// The zoom the eye's walk asks a node of planes `p` at its nearest point, as walk.wgsl
/// asks it (`cast::node_zooms`).
fn asked_near(p: Planes) -> u32 {
  let range = vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u));
  if (range.x == range.y) {
    return range.x;
  }
  return zoom_at(squared(min(max(p.lo, vec3<f32>(0.0)), p.hi)), range);
}

/// Whether the eye's ray from `o` stopped on `k` whole, coarser than it asked there
/// (`record::stopped_whole`).
fn stopped_whole(k: Node, o: vec3<i32>) -> bool {
  return k.zoom < asked_near(kept_planes(k, o));
}

/// Whether `k`, `whole` where it stopped whole, is the node the eye's walk from `o` stops on:
/// it answers a cell and its parent does not (`record::fresh`).
fn kept_fresh(k: Node, whole: bool, o: vec3<i32>) -> bool {
  if (!whole && k.zoom < asked_near(kept_planes(k, o))) {
    return false;
  }
  if (k.zoom == 0u) {
    return true;
  }
  let zoom = k.zoom - 1u;
  let parent = Node(zoom, k.corner & vec3<u32>(~under(zoom)), k.roots);
  return zoom < asked_near(kept_planes(parent, o));
}

/// Whether the node of `zoom`, `whole` where some ray stopped on it whole, is the node a
/// light's ray asking `band` stops on: at the band, or coarser where the query answers it a
/// cell whole (pxl-engine `record::band_fresh`).
fn band_fresh(zoom: u32, whole: bool, band: u32) -> bool {
  return zoom == band || (whole && zoom < band);
}
