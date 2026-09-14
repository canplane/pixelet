// @pxl/web/module/world/billboard/frustum_budget.ts

/**
 * Shared visibility budget for world-anchored billboards (tier 3 — see
 * issue/20260907-world-rendering-tiers.md). A consumer with more
 * candidates than it can afford to mount (POI today; a future multiplayer
 * chat-bubble-per-character feature would hit the same shape) calls
 * select_visible_billboards once per selection tick:
 *
 * 1. Real camera-frustum membership, tested against a sphere around the
 *    anchor rather than the bare anchor point — distance/FAR_END fading
 *    (distance_fade.ts) still owns within-frustum fade; this is a coarser
 *    pre-filter.
 *
 *    The sphere matters at an oblique camera. A billboard is drawn *above*
 *    its anchor on screen, so an anchor that has just dropped below the
 *    bottom edge still has its pill in full view — and at an oblique polar
 *    the bottom edge is where the *nearest* anchors are (the camera sits on
 *    the target's -Y side and looks down at it; see camera/orbit.ts). Point
 *    membership therefore deleted the closest badges first while distant
 *    ones near the horizon stayed, and zooming in made it worse by pushing
 *    more anchors past that edge. Looking straight down the artifact
 *    disappears, because there the anchor and its pill share a screen
 *    position.
 * 2. If still over max_count: nearest first, with a stable hash of each
 *    candidate's own id breaking ties.
 *
 * The ranking used to be the hash alone, chosen so the survivors would not
 * flicker frame to frame the way Math.random() would. It achieved that and
 * still produced the worst artifact this file has had: the cap is a cliff.
 * At or under max_count every candidate draws; one candidate over, the whole
 * set is ranked by a value unrelated to where anything is and cut. So the
 * smallest zoom-out that pulled an 81st POI into the frustum could delete a
 * badge in the middle of the screen while keeping one at the edge, with
 * nothing in front of either.
 *
 * Distance restores monotonicity: widening the view can only append farther
 * candidates, so the ones already on screen keep their slots and what falls
 * off the end is the farthest — which is also the smallest and the least
 * legible. The original objection to distance was that it always favours the
 * same cluster, and that is true and is the point: the near cluster is the
 * one being looked at.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { mat4_transform_point } from "@xpute/core/kit/mat4.ts";
import { GridIndex, NO_ROOM, PLACED } from "@xpute/core/ds/grid_index.ts";
import { view, view_sees_sphere } from "@/module/world/view.ts";
import { distance_fade_far } from "./distance_fade.ts";
import type { BillboardBatch } from "./batch.ts";

const $pos = { x: 0, y: 0, z: 0 };
const $ndc = { x: 0, y: 0, z: 0 };

/** Candidates the last pass could neither draw nor attribute to something
 * drawn — the budget or the grid ran out. Nonzero means the map is quietly
 * short of places, which is what absorption exists to prevent. */
let dropped: u32 = 0;
export function billboard_dropped(): u32 {
  return dropped;
}

/**
 * Placed badges in screen pixels, so a candidate can find out whether its
 * pixels are already taken. Sized once for a viewport larger than anything
 * this will meet; `begin` takes the live size, so a resize costs nothing.
 */
const MAX_VIEWPORT_PX = 4096;
const $screen = new GridIndex(MAX_VIEWPORT_PX, MAX_VIEWPORT_PX, 512);

/**
 * How far around its anchor a billboard can reach, as a fraction of its own
 * distance from the camera.
 *
 * A fraction rather than a world length because the badge shader holds a
 * billboard at a roughly constant *screen* size, so its world extent grows
 * with distance. This is that angular size with room to spare — generous on
 * purpose, since the cost of keeping one badge too many is one sprite and
 * the cost of dropping one too early is the bug above.
 */
const ANCHOR_REACH: f64 = 0.08;

/**
 * The fade level below which a billboard is not worth a draw call.
 *
 * `far` is the drawn size as a fraction of full and it is also the opacity
 * (distance_fade.ts), so this is one number saying both "a tenth of the size"
 * and "a tenth of the alpha" — a hundred-pixel badge at ten by three,
 * barely tinting what is behind it. Cutting here removes the outer shell of
 * the reach and changes nothing anyone can see.
 */
const MIN_LEGIBLE_FAR: f64 = 0.12;

/**
 * The candidates that survive the frustum, the budget and the collision
 * pass, as indices into `batch` written to `out`.
 *
 * Indices rather than items: this pass never learns what a candidate *is*,
 * which is what lets POI badges, chat bubbles and actors share it while
 * storing entirely different things. The caller maps `batch.src[out[i]]`
 * back to its own data. The batch's coordinates are in the root frame
 * (module/world/view.ts), and the view as it stands is what they are
 * measured against.
 */
export function select_visible_billboards(batch: BillboardBatch, max_count: u32, viewport_w: f64, viewport_h: f64, out: Uint32Array): u32 {
  const n = select_begin(batch, viewport_w, viewport_h);
  return select_place(batch, max_count, viewport_w, viewport_h, out, n);
}

/** The selection's cursor, so a placement job can run the greedy pass a
 * range of candidates per unit: how many survived the frustum, how far the
 * placement has got, how many it has kept. One selection at a time. */
let $n: u32 = 0;
let $k: u32 = 0;
let $kept: u32 = 0;

/**
 * The frustum pass and the sort — one unit. Returns how many candidates
 * survived, which is what `select_place` walks.
 */
export function select_begin(batch: BillboardBatch, viewport_w: f64, viewport_h: f64): u32 {
  dropped = 0;
  // Hoisted for the whole pass. Safe because nothing here pushes, and a
  // lane's backing store is only replaced by growth (batch.ts).
  const { local_x, local_y, local_z, world_x, world_y, world_z, d2, order, tiebreak, absorbed } = batch;

  const eye = view.eye;

  let n: u32 = 0;
  for (let i: u32 = 0; i < batch.count; i++) {
    // Through the root gives the point the frustum test needs, the point
    // the distance is measured from, and the point the screen box is
    // projected from — done once rather than three times, and kept in the
    // lanes so the placement pass below does not redo it.
    mat4_transform_point($pos, view.root, local_x[i], local_y[i], local_z[i]);
    const dist2 = ($pos.x - eye.x) ** 2 + ($pos.y - eye.y) ** 2 + ($pos.z - eye.z) ** 2;
    if (!view_sees_sphere($pos.x, $pos.y, $pos.z, Math.sqrt(dist2) * ANCHOR_REACH)) continue;

    world_x[i] = $pos.x;
    world_y[i] = $pos.y;
    world_z[i] = $pos.z;
    d2[i] = dist2;
    absorbed[i] = 0;
    order[n++] = i;
  }

  // Squared distance: only the order matters, so the square root does not.
  // The tiebreak decides nothing on its own — it keeps two candidates at
  // equal distance from swapping places frame to frame.
  //
  // Distance stays the ordering for the reason this file's own doc comment
  // gives: it is what makes widening the view append rather than reshuffle.
  // The collision pass below only decides who yields to whom, and ordering
  // that by general notability instead would reintroduce the exact artifact
  // that comment describes — a badge in the middle of the screen deleted to
  // keep one at the edge.
  //
  // `subarray` is the one object this pass still creates per call, since a
  // sort needs the exact length. Sorting the full capacity to avoid it would
  // trade one small view for an order of magnitude more comparisons.
  order.subarray(0, n).sort((a, b) => d2[a] - d2[b] || tiebreak[a] - tiebreak[b]);

  $screen.begin(viewport_w, viewport_h);
  $n = n;
  $k = 0;
  $kept = 0;
  return n;
}

/**
 * Greedy placement in distance order, which is Mapbox GL's shape: compute
 * the screen box, take the space if it is free, otherwise this candidate
 * does not draw. Without it the budget was a distance cut that let two
 * badges sit on top of each other and dropped a third that had room, since
 * nothing anywhere asked whether they overlapped.
 *
 * Runs candidates up to `k_end` of the order `select_begin` sorted and
 * returns how many are kept so far; call again with a larger `k_end` to
 * continue. A projection per candidate — forty milliseconds for the two
 * and a half thousand a dense area offers, on a phone (2026-09-13) — is
 * why this is resumable rather than one call.
 */
export function select_place(
  batch: BillboardBatch,
  max_count: u32,
  viewport_w: f64,
  viewport_h: f64,
  out: Uint32Array,
  k_end: u32,
): u32 {
  const { world_x, world_y, world_z, d2, order, w_px, collide_px, anchor_px, absorbed } = batch;
  let kept = $kept;
  const end = Math.min(k_end, $n);

  for (let k = $k; k < end; k++) {
    const i = order[k];

    const far = distance_fade_far(Math.sqrt(d2[i]));
    // Not `far <= 0`. A badge is drawn at `logical_w * far`, so the last
    // stretch before FAR_END is ten pixels by three at a tenth opacity, and
    // then one by zero — invisible, and small enough that hundreds of them
    // fit without ever colliding. In the densest part of the bake the shell
    // between MIN_LEGIBLE_FAR and FAR_END holds around nine hundred POI, so
    // this is the difference between a budget bounded by what the screen can
    // show and one bounded by how much world is in reach.
    //
    // The old cap hid this: eighty slots handed out nearest-first never
    // reached the far ones. Absorption removed the cap as the mechanism, and
    // that is the right thing to have removed — but it also removed what was
    // accidentally doing this job, so the job has to be done on purpose.
    //
    // Not absorbed either, for the same reason nothing past FAR_END is: this
    // is where the layer stops, and a count is only honest about what the
    // layer is looking at.
    if (far < MIN_LEGIBLE_FAR) continue;

    mat4_transform_point($ndc, view.view_proj, world_x[i], world_y[i], world_z[i]);
    const sx = ($ndc.x * 0.5 + 0.5) * viewport_w;
    const sy = (-$ndc.y * 0.5 + 0.5) * viewport_h;

    // Canvas Y and screen Y both grow downward, so the anchor's canvas
    // offset reads straight across: the box's top sits that far above the
    // projected anchor, and it is `collide_px` tall rather than the full
    // drawn height — see BuiltBillboard.collide_h.
    const w = w_px[i] * far;
    const x0 = sx - w / 2;
    const y0 = sy - anchor_px[i] * far;
    const x1 = x0 + w;
    const y1 = y0 + collide_px[i] * far;

    // No early exit once the budget is full. A candidate that is neither
    // placed nor counted has been silently deleted, which is the thing this
    // pass exists to stop doing — so past the budget it still asks who owns
    // the space, it just cannot take any.
    const room = kept < max_count && kept < out.length;
    const blocker = room ? $screen.place(x0, y0, x1, y1) : $screen.first_hit(x0, y0, x1, y1);

    if (blocker === PLACED) {
      // Free pixels. Either it stands alone, or it is about to become the
      // place where everything behind it is counted.
      if (room) out[kept++] = i;
      else dropped++;
      continue;
    }
    if (blocker === NO_ROOM) {
      dropped++;
      continue;
    }

    // The whole point. This candidate is not drawn, and it is not gone
    // either: the badge standing on its pixels now stands for it too. A
    // group's badge is a count rather than its first member's name, so
    // nothing here is a representative — `blocker` donated a position and
    // its own identity goes with everyone else's.
    //
    // `blocker` is the id GridIndex handed out on insert, which is the order
    // things were placed in, which is this loop's own `kept` at the time —
    // so it indexes `out` directly.
    absorbed[out[blocker]]++;
  }
  $k = end;
  $kept = kept;
  return kept;
}

/** How many candidates the selection has left to place. */
export function select_remaining(): u32 {
  return $n - $k;
}
