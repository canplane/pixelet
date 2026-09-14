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
  GHOST = 0x0c,
  PUCK = 0x0d,
  BILLBOARD = 0x0e,
  ACTOR = 0x0f,
  DEBUG = 0x10,
  ENV = 0x11,
  MODEL = 0x12,
  CHANGE = 0x13,
  FRAME = 0x14,
  DRAW = 0x15,
  SHADER = 0x16,
}

/** The hover ghost's own verbs. */
export const enum GhostMinor {
  BOX = 0x10,
  DECOR = 0x11,
}

/** The location puck's own verb. */
export const enum PuckMinor {
  BEACON = 0x10,
}

/** A billboard set's own verbs. */
export const enum BillboardMinor {
  PLACE = 0x10,
  FOLLOW = 0x11,
}

/** An actor's own verb. */
export const enum ActorMinor {
  PLAY = 0x10,
}

/** A model's own verbs. */
export const enum ModelMinor {
  JOINT = 0x10,
  CLIP = 0x11,
  CHANNEL = 0x12,
  SEAL = 0x13,
}

/** The environment's own verbs. */
export const enum EnvMinor {
  OBSERVE = 0x10,
  LANES = 0x11,
}

/** The change log's own verb. */
export const enum ChangeMinor {
  LANES = 0x10,
}

/** The shaders' own verb. */
export const enum ShaderMinor {
  PIPELINE = 0x10,
}

/** An object store's own verb. */
export const enum ObjectMinor {
  REINDEX = 0x10,
  LANES = 0x11,
  CELL = 0x12,
  COLUMNS = 0x13,
  STAT = 0x14,
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
  BUILD = 0x14,
  DECOR = 0x15,
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

  // ---- the scene: what is drawn beside the chunks ----
  //
  // A lane of floats crosses in the packet, not by address: the glue's
  // scratch is given back as soon as the command is sent, and the command
  // is applied later.

  /** [x f64, y f64, z f64, size f64, value u32, rgb u32, coplanar bool] — the
   * hover ghost as a box, its min corner at (x, y, z) */
  GHOST_BOX = 0x0c10,
  /** [x f64, y f64, z f64, group u32, rotation f64] — the hover ghost as one
   * instance of a decor group */
  GHOST_DECOR = 0x0c11,
  /** no packet — the ghost gone */
  GHOST_DELETE = 0x0c04,
  /** [x f64, y f64, z f64, zone f32[7 an instance], snap_dist f64, fog_far
   * f64] — the puck's zone about its centre */
  PUCK_UPDATE = 0x0d03,
  /** [offset_x f64, offset_y f64, night bool] */
  PUCK_BEACON = 0x0d10,
  /** no packet — the puck gone */
  PUCK_DELETE = 0x0d04,
  /** tag call · [order u32] → result: the set's id */
  BILLBOARD_CREATE = 0x0e01,
  /** tag set · [instances f32[10 an instance]], relative to its anchor */
  BILLBOARD_UPDATE = 0x0e03,
  /** tag set · [x f64, y f64, z f64] — stood at a point */
  BILLBOARD_PLACE = 0x0e10,
  /** tag set · [actor u32, lift f64] — stood above an actor */
  BILLBOARD_FOLLOW = 0x0e11,
  /** tag set · no packet */
  BILLBOARD_DELETE = 0x0e04,
  /** tag call · [model u32] → result: the actor's id, or -1 */
  ACTOR_CREATE = 0x0f01,
  /** tag actor · [x f64, y f64, z f64, rotation_z f64, visible bool] */
  ACTOR_UPDATE = 0x0f03,
  /** tag actor · [clip u32, fade_s f32] */
  ACTOR_PLAY = 0x0f10,
  /** tag actor · no packet */
  ACTOR_DELETE = 0x0f04,
  /** [x f64, y f64, z f64, vertices f32[6 a vertex]] — the dev gizmos'
   * segments about an origin */
  DEBUG_UPDATE = 0x1003,
  /** [theme f32[34]] — the theme the environment eases toward */
  ENV_UPDATE = 0x1103,
  /** [lat f64, lon f64, unix_ms f64] — the observer and the wall clock */
  ENV_OBSERVE = 0x1110,

  // ---- models: a mesh block the glue writes, and its skeleton and clips ----

  /** tag call · [vertex_cnt u32, index_cnt u32, joint_cnt u32] → result: the
   * model's id, or -1 when the mesh pool refused; with an id, [block_at u32]
   * — where the glue writes the vertices and indices, in the module's
   * memory */
  MODEL_CREATE = 0x1201,
  /** tag call · [model u32] → result: the renderer's handle for its mesh
   * block, 0 before it is sealed */
  MODEL_READ = 0x1202,
  /** tag model · [joint u32, parent i32, pose f32[42]: rest (10), what
   * stands above a root (16), inverse bind (16)] */
  MODEL_JOINT = 0x1210,
  /** tag call · [model u32, duration f32] → result: the clip's index */
  MODEL_CLIP = 0x1211,
  /** tag model · [clip u32, joint u32, path u32 (0 translation, 1 rotation,
   * 2 scale), step bool, times f32[], values f32[]] */
  MODEL_CHANNEL = 0x1212,
  /** tag model · [scale f32] — complete: its block goes to the GPU */
  MODEL_SEAL = 0x1213,

  // ---- the change log, the chunks' rebuilds now, the day ----

  /** tag call · [topic u32, key u64] → result: the key's new revision */
  CHANGE_UPDATE = 0x1303,
  /** tag call · [topic u32, key u64] → result: the key's revision, 0 for
   * one never bumped */
  CHANGE_READ = 0x1302,
  /** [topic u32, key u64] — the key's revision forgotten */
  CHANGE_DELETE = 0x1304,
  /** tag chunk · no packet — its layout built to the end now: a placement
   * shows in the frame it was made */
  CHUNK_BUILD = 0x0114,
  /** tag chunk · [show_decor bool] — its decor rebuilt now for the layout it
   * shows; the tiers untouched */
  CHUNK_DECOR = 0x0115,
  /** tag call · [lat f64, lon f64, day_start_ms f64] → [sunrise f64, sunset
   * f64], in hours after the day's start */
  ENV_READ = 0x1102,

  // ---- where the lanes the shell reads in place are ----
  //
  // Asked once, or once a frame for the draw lists: the answer is addresses
  // in the module's memory, which never grows.

  /** tag call · no packet → [frame_at u32 (FRAME_WORDS doubles the glue
   * writes before a tick), uniform u32[14]: the frame uniform's floats,
   * then where each field starts] */
  FRAME_READ = 0x1402,
  /** tag call · no packet → [census_at u32 (13 doubles), stat_at u32 (11
   * words)] */
  CHUNK_READ = 0x0102,
  /** tag call · no packet → [out_at u32]: the environment's ENV_FLOATS */
  ENV_LANES = 0x1111,
  /** tag call · no packet → [topic_at u32, key_at u32, revision_at u32,
   * head_at u32, ticks_at u32, capacity u32]: the change log's lanes */
  CHANGE_LANES = 0x1310,
  /** tag call · no packet → [row u32[10]]: the renderer's lists taken — the
   * uploads' address and count, the frees', the draws', the uniforms', the
   * rewrites' — valid until the next take */
  DRAW_READ = 0x1502,
  /** tag call · [kind u32] → result: the WGSL's length in bytes, 0 for a
   * kind that is not one; with one, [at u32] */
  SHADER_READ = 0x1602,
  /** tag call · [id u32] → [record u32[]]: the pipeline's record, empty for
   * an id that is not one */
  SHADER_PIPELINE = 0x1610,

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
  /** tag call · [key u64, band_z i32, x u32, y u32, z u32] → result: the
   * state stored at the leaf, VOID where nothing is */
  OBJECT_READ = 0x0802,
  /** tag call · [key u64] → result: the chunk's entry count, 0 where it
   * holds none; with entries, [band_at u32, pos_at u32, state_at u32] — the
   * addresses of its lanes, valid until the next write or reindex */
  OBJECT_LANES = 0x0811,
  /** tag call · [key u64, cx u32, cy u32, cz i32] → result 1 where the cell
   * holds entries, 0 where not; with them, [start u32, count u32, solid u32] */
  OBJECT_CELL = 0x0812,
  /** tag call · no packet → [keys u64[]]: the columns holding objects, in
   * the store's order */
  OBJECT_COLUMNS = 0x0813,
  /** tag call · no packet → [chunks u32, bands u32, cells u32] */
  OBJECT_STAT = 0x0814,
}
