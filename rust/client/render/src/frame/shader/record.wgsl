// pxl-render/record.wgsl

// The record's reads of the node cache (pxl-engine record.rs). They give the node an entry
// holds, placed as a walk from a given origin holds it; whether that node is the one the eye's
// walk or a light's ray stops on, which makes the entry fresh; and which entry draws a node a
// ray stopped on. The module that includes this part binds the node cache and reads its words
// through `node_cache_word`. pxl-engine record.rs names the passes that write the record and
// their order.

/// The value that stands for no slot and no place.
const NO_ENTRY: u32 = 0xffffffffu;

fn record_frame() -> u32 {
  return node_cache_word(FRAME);
}

/// A node as a walk holds it (pxl-engine `cast::Node`), except that `roots` counts over the
/// whole world: the walk's roots, which count from the origin's root, plus the origin's own.
struct Node {
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
}

/// The root that holds the origin `o` on each axis (pxl-engine `cast::roots_of`).
fn origin_roots(o: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(o.x >> 31u, 0i, i32(u32(o.z) + Z_START) >> 31u);
}

/// The node at `zoom` whose world index is `c`, as a walk from the origin `o` holds it
/// (pxl-engine `cast::Node::of_idx`). Its corner comes from the index's low bits. On each axis
/// its root is the origin's or one to either side: the one whose copy of the node has its
/// middle within half a root of the origin.
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

/// The node the entry in slot `slot` holds, as a walk from the origin `o` holds it.
fn entry_node(slot: u32, o: vec3<i32>) -> Node {
  let n = entry_at(slot) + W_NODE;
  let c = vec3<i32>(i32(node_cache_word(n + 1u)), i32(node_cache_word(n + 2u)), i32(node_cache_word(n + 3u)));
  return node_of_idx(o, node_cache_word(n), c);
}

/// The kind of the entry in slot `slot` (pxl-engine `node_cache`). A free slot and a claimed
/// one hold no node, and count as `KIND_SPLIT`, which no draw takes.
fn entry_kind(slot: u32) -> u32 {
  let key = node_cache_word(key_at(slot));
  if (key == 0u || key == CLAIMED || (key & CLAIM_MASK) == CLAIM_FAR) {
    return KIND_SPLIT;
  }
  return node_cache_word(entry_at(slot) + W_KIND) & KIND_MASK;
}

/// The slot of the entry that draws the node `c` at `zoom` that a ray stopped on, where the
/// node answers the asset `asset`, or is a lack where `lacks` (pxl-engine `record`). Among the
/// entries of the node's bucket that hold it with that answer, this is the one that already
/// draws, with a face set, or else the first; `NO_ENTRY` where there is none. No pass that
/// calls this writes a key, so every call in one pass finds the same entry.
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

/// A box's low and high planes, in units from an origin (pxl-engine cast.rs `bounds`).
struct Aabb {
  lo: vec3<f32>,
  hi: vec3<f32>,
}

fn node_planes(k: Node, o: vec3<i32>) -> Aabb {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let roots = k.roots - origin_roots(o);
  return Aabb(units(k.corner, og, roots), far_of(k.corner, under(k.zoom), og, roots));
}

/// The zoom at which the eye's walk asks a node with planes `p`, at the node's nearest point,
/// as walk.wgsl asks it (pxl-engine `cast::node_zooms`).
fn asked_near(p: Aabb) -> u32 {
  let range = vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u));
  if (range.x == range.y) {
    return range.x;
  }
  return zoom_at(squared(min(max(p.lo, vec3<f32>(0.0)), p.hi)), range);
}

/// Whether the eye's ray from `o` stopped on `k` whole, that is, on a node coarser than the
/// zoom it asked there (pxl-engine `record::stopped_whole`).
fn stopped_whole(k: Node, o: vec3<i32>) -> bool {
  return k.zoom < asked_near(node_planes(k, o));
}

/// Whether `k` is the node the eye's walk from `o` stops on (pxl-engine `record::fresh`). It is
/// where `k` answers a cell, at its zoom or because `whole` says some ray stopped on it whole,
/// and its parent does not.
fn is_fresh(k: Node, whole: bool, o: vec3<i32>) -> bool {
  if (!whole && k.zoom < asked_near(node_planes(k, o))) {
    return false;
  }
  if (k.zoom == 0u) {
    return true;
  }
  let zoom = k.zoom - 1u;
  let parent = Node(zoom, k.corner & vec3<u32>(~under(zoom)), k.roots);
  return zoom < asked_near(node_planes(parent, o));
}

/// Whether a node at `zoom` is the node a light's ray asking zoom `band` stops on (pxl-engine
/// `record::band_fresh`). The ray stops at the band, or on a coarser node where the query
/// answers that node as a whole cell; `whole` says some ray stopped on it so.
fn band_fresh(zoom: u32, whole: bool, band: u32) -> bool {
  return zoom == band || (whole && zoom < band);
}
