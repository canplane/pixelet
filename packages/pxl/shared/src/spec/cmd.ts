// @pxl/shared/spec/cmd.ts

/**
 * The shell's and the kernel's commands, by number (@xpute/core/ipc/cmd.ts:
 * `major << 8 | minor`), and the packet each carries — an XTP branch whose
 * children are listed in order. A message's tag is its subject: a chunk's
 * id where there is one, so no packet repeats it. A command with no
 * children carries no packet.
 *
 * The same number is a command on the submission ring and a signal on the
 * completion ring (its `RES` clear): a chunk the shell creates is a chunk
 * it registers, a chunk the kernel creates is one it mounted. The direction
 * is the ring.
 */

/** What a command is about. */
export const enum Major {
  NOP = 0x00,
  CHUNK = 0x01,
  PAGE = 0x02,
  EDIT = 0x03,
  TIER = 0x04,
  RUNS = 0x05,
  DECOR = 0x06,
  RAY = 0x07,
  OBJECT = 0x08,
  STATE = 0x09,
  TERRAIN = 0x0a,
  DECOR_KIND = 0x0b,
}

/** An object store's own verb. */
export const enum ObjectMinor {
  REINDEX = 0x10,
}

/** A decor kind's own verb. */
export const enum DecorKindMinor {
  MODEL = 0x10,
}

/** A chunk's own verbs, past the four every subject has. */
export const enum ChunkMinor {
  WANT = 0x10,
  VISIBLE = 0x11,
  LAYOUT = 0x12,
  BLEND = 0x13,
}

export const enum Cmd {
  /** Replies with its packet's first child, a u32, or 0 — the rings' own
   * test. */
  NOP = 0x0000,

  // ---- shell → kernel ----

  /** tag chunk · [base u64, tx0 u32, ty0 u32, base_x i32, base_y i32,
   * stable_hash u32] */
  CHUNK_CREATE = 0x0101,
  /** tag chunk · [page: [land u32, water u32, structure u32, transport u32,
   * ele u32] addresses | null, show_structures bool] */
  CHUNK_UPDATE = 0x0103,
  /** tag chunk · no packet */
  CHUNK_DELETE = 0x0104,
  /** tag chunk · [first u32, second u32], a step or 0 — the builder's ask,
   * past the scheduler, for the tests */
  CHUNK_WANT = 0x0110,
  /** [page u64] */
  PAGE_CREATE = 0x0201,
  /** [page u64] */
  PAGE_DELETE = 0x0204,
  /** [hide: [column u64, band i32, pos u32] | null, carve: [column u64,
   * lx f32, ly f32, lz f32, size f32] | null] */
  EDIT_UPDATE = 0x0303,
  /** [key u64, band_z i32, x u32, y u32, z u32, level i32, value u32] — one
   * leaf; the chunk's entries are not re-derived until OBJECT_REINDEX */
  OBJECT_UPDATE = 0x0803,
  /** no packet — the whole store emptied */
  OBJECT_DELETE = 0x0804,
  /** [key u64] — the chunk's entries and cells re-derived, and the change
   * recorded in the change log: the column in both topics, every cell
   * whose entries changed */
  OBJECT_REINDEX = 0x0810,
  /** [state u32, kind u32 (1 solid, 2 air, 3 decor), material u32 (packed
   * 0xCC_RRGGBB), layer u32] — what a leaf's state paints as */
  STATE_UPDATE = 0x0903,
  /** [base: [r u32, g u32, b u32, layer u32, class u32], channels: [land,
   * water, structure, transport: [class u32, rgb u8[768], has_color u8[256],
   * layer u8[256], has_layer u8[256]]]] — every table a cell's material is
   * read from, whole */
  TERRAIN_UPDATE = 0x0a03,
  /** [kinds: [[tag u64, density f64, variant_count u32, align_transport
   * bool, extent f64, decor_value i32 (-1 none), rules: [land, water,
   * structure, transport: [mode u32 (0 any, 1 void, 2 one of), types u8[]]]]]]
   * — every decor kind, in group order, replacing what was taught */
  DECOR_KIND_UPDATE = 0x0b03,
  /** [group u32, model u32] — the model a decor group is drawn with, for the
   * hit test */
  DECOR_KIND_MODEL = 0x0b10,

  // ---- kernel → shell (signals; CHUNK_CREATE and CHUNK_DELETE above too) ----
  //
  // CHUNK_CREATE: tag chunk · [page u64, base_x i32, base_y i32] — mounted
  // CHUNK_DELETE: tag chunk · no packet — unmounted

  /** tag chunk · [on bool] */
  CHUNK_VISIBLE = 0x0111,
  /** tag chunk · [primary u32, secondary u32 (0 for none), ready bool] */
  CHUNK_LAYOUT = 0x0112,
  /** tag chunk · [primary f32, secondary f32, decor_secondary bool] */
  CHUNK_BLEND = 0x0113,
  /** tag chunk · [generation u32, step u32, at u32, bytes u32, vertex_cnt u32,
   * index_cnt u32] — a tier built, in its mesh block */
  TIER_CREATE = 0x0401,
  /** tag chunk · [step u32] — a tier evicted */
  TIER_DELETE = 0x0404,
  /** tag chunk · [bytes u32] */
  RUNS_UPDATE = 0x0503,
  /** tag chunk · [step u32, at u32, bytes u32] */
  DECOR_CREATE = 0x0601,

  // ---- queries: ACKREQ, answered in the turn they are asked ----
  //
  // A ray: [ray f64[9]: origin_x, origin_y (the map origin, units), ox, oy,
  // oz, dx, dy, dz, max_dist], in the root frame. The reply's result is 1
  // on a hit and 0 on none.

  /** tag call · [ray f64[9]] → [chunk u32, band u32, pos u32, t f64]: the
   * placed decor entry struck */
  DECOR_READ = 0x0602,
  /** tag call · [ray f64[9]] → [hit f64[14]: material class, normal ×3,
   * point ×3, t, box min ×3, box max ×3] */
  RAY_READ = 0x0702,
}
