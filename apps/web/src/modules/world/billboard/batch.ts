// @pxl/web/modules/world/billboard/batch.ts

/**
 * One frame's billboard candidates, as lanes rather than objects.
 *
 * What this replaces was an array of `{item, id, local_x, local_y, local_z,
 * ...}` built fresh every camera tick, handed to a selector that built a
 * second array of its own, which returned a third. Three arrays and three
 * objects per candidate, per tick, for a set that is thrown away at the end
 * of it — which is precisely the shape deck.gl names as the expensive one
 * ("avoid creating per-item objects in accessors") and the shape Figma
 * removes by keeping objects in preallocated typed arrays.
 *
 * The lanes are also the only form that crosses to WASM. A `Vec<f64>` is a
 * `Float64Array`; an array of JS objects has no counterpart in linear memory
 * at all. The index structures on either side of this — XTP bytes below,
 * the spatial hash above — already survive that move, and this was the layer
 * between them that did not.
 *
 * `src` is deliberately not the caller's data but an index back into it: a
 * batch knows where a candidate came from and nothing about what it is,
 * which is what keeps POI, chat bubbles and actors able to share one
 * selector while storing entirely different things.
 *
 * `tiebreak` is a precomputed number, not an id to hash at selection time.
 * The value is stable per candidate for as long as it exists, so hashing it
 * once where the candidate is created costs nothing per frame and removes
 * string work from the tick entirely.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";

/**
 * A frame's worth of candidates is not a residency set, so it grows to what
 * it meets rather than refusing past a ceiling. Dropping here would be a
 * badge missing from the screen for no reason a viewer could see, and the
 * high-water mark is a property of the densest place visited rather than of
 * how long the session has run.
 *
 * The lanes are `Vector` for that growth alone; the batch keeps the length.
 * Eight lanes with eight lengths is eight chances to diverge, and `set`'s
 * own auto-extension would do exactly that. Same arrangement
 * chunk_mesher.ts's CO_* lanes already use, with the lockstep made explicit
 * instead of being five `truncate` calls that have to stay together.
 */
const INIT_CAP: u32 = 1 << 10;

export class BillboardBatch {
  count: u32 = 0;

  /** Position relative to local_world_frame's own origin — see AnchorNode's
   * own local-offset math (frame/coord.ts). */
  private readonly _local_x = new Vector(Float64Array, { init_cap: INIT_CAP });
  private readonly _local_y = new Vector(Float64Array, { init_cap: INIT_CAP });
  private readonly _local_z = new Vector(Float64Array, { init_cap: INIT_CAP });

  /** Full-strength drawn width in CSS pixels, before the distance fade. */
  private readonly _w_px = new Vector(Float32Array, { init_cap: INIT_CAP });
  /** Height that must not be overlapped, which is not the drawn height —
   * see BuiltBillboard.collide_h. */
  private readonly _collide_px = new Vector(Float32Array, { init_cap: INIT_CAP });
  /** Canvas-space Y of the anchor, in the same units as `collide_px`. */
  private readonly _anchor_px = new Vector(Float32Array, { init_cap: INIT_CAP });

  /** Stable per candidate, used only to keep two at equal distance from
   * swapping places between frames. */
  private readonly _tiebreak = new Vector(Float64Array, { init_cap: INIT_CAP });

  /** The caller's own index for this candidate. */
  private readonly _src = new Vector(Uint32Array, { init_cap: INIT_CAP });

  /* Filled by the selector rather than the producer. They live here so the
   * whole pass has one capacity and one allocation site instead of a second
   * set of scratch arrays that has to be kept the same size. */

  /** World position, resolved once from the local one. */
  private readonly _world_x = new Vector(Float64Array, { init_cap: INIT_CAP });
  private readonly _world_y = new Vector(Float64Array, { init_cap: INIT_CAP });
  private readonly _world_z = new Vector(Float64Array, { init_cap: INIT_CAP });
  /** Squared distance to the eye. Squared because only the order is used. */
  private readonly _d2 = new Vector(Float64Array, { init_cap: INIT_CAP });
  /** Candidate indices that passed the frustum, in the order they will be
   * offered to the placement pass. */
  private readonly _order = new Vector(Uint32Array, { init_cap: INIT_CAP });
  /** How many candidates were folded into this one because they wanted the
   * pixels it took. Meaningful only for the ones the selector kept, and zero
   * there means it stands alone. */
  private readonly _absorbed = new Vector(Uint32Array, { init_cap: INIT_CAP });

  /* Growth replaces the backing store (arena.ts's own `_grow`), so a lane is
   * read through the batch and only ever hoisted for the length of a loop
   * that cannot push. That constraint is why these are accessors rather than
   * public fields: when instanced rendering starts holding a lane as a GPU
   * attribute, the reallocation it has to notice happens in one place. */
  get local_x(): Float64Array {
    return this._local_x.mem;
  }
  get local_y(): Float64Array {
    return this._local_y.mem;
  }
  get local_z(): Float64Array {
    return this._local_z.mem;
  }
  get w_px(): Float32Array {
    return this._w_px.mem;
  }
  get collide_px(): Float32Array {
    return this._collide_px.mem;
  }
  get anchor_px(): Float32Array {
    return this._anchor_px.mem;
  }
  get tiebreak(): Float64Array {
    return this._tiebreak.mem;
  }
  get src(): Uint32Array {
    return this._src.mem;
  }
  get world_x(): Float64Array {
    return this._world_x.mem;
  }
  get world_y(): Float64Array {
    return this._world_y.mem;
  }
  get world_z(): Float64Array {
    return this._world_z.mem;
  }
  get d2(): Float64Array {
    return this._d2.mem;
  }
  get order(): Uint32Array {
    return this._order.mem;
  }
  get absorbed(): Uint32Array {
    return this._absorbed.mem;
  }

  /** Backing capacity, which is the high-water mark rather than a limit. */
  get cap(): u32 {
    return this._src.cap;
  }

  reset(): void {
    this.count = 0;
  }

  push(
    src: u32,
    local_x: f64,
    local_y: f64,
    local_z: f64,
    w_px: f64,
    collide_px: f64,
    anchor_px: f64,
    tiebreak: f64,
  ): void {
    const i = this.count++;
    // Every lane, including the selector's: one capacity for the whole pass
    // means the selector never has to check a second length. `reserve` is
    // read as "capacity for this many" here, since a lane's own len stays 0.
    if (i >= this._src.cap) this._reserve(i + 1);

    this._local_x.mem[i] = local_x;
    this._local_y.mem[i] = local_y;
    this._local_z.mem[i] = local_z;
    this._w_px.mem[i] = w_px;
    this._collide_px.mem[i] = collide_px;
    this._anchor_px.mem[i] = anchor_px;
    this._tiebreak.mem[i] = tiebreak;
    this._src.mem[i] = src;
  }

  private _reserve(n: u32): void {
    this._local_x.reserve(n);
    this._local_y.reserve(n);
    this._local_z.reserve(n);
    this._w_px.reserve(n);
    this._collide_px.reserve(n);
    this._anchor_px.reserve(n);
    this._tiebreak.reserve(n);
    this._src.reserve(n);
    this._world_x.reserve(n);
    this._world_y.reserve(n);
    this._world_z.reserve(n);
    this._d2.reserve(n);
    this._order.reserve(n);
    this._absorbed.reserve(n);
  }
}

/**
 * FNV-1a into [0,1). Same shape as chunk_stage.tsx's own module-private
 * blend_hash — duplicated rather than imported to avoid coupling to that
 * file's internals, and used here once per candidate's lifetime rather than
 * once per frame.
 */
export function stable_hash(key: string): f64 {
  let hash = 2166136261;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) / 0x1_0000_0000;
}
