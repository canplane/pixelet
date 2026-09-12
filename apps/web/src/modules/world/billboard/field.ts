// @pxl/web/modules/world/billboard/field.ts

/**
 * Every billboard of one tier, as one scene node and a pool of sprites.
 *
 * What this replaces mounted a React subtree per badge — a component, an
 * AnchorNode with its own Group and portal, a Billboard, a SpriteBillboard —
 * so a few hundred badges were a few thousand elements reconciled on every
 * placement, and a changed key destroyed and rebuilt a Sprite, a
 * SpriteMaterial and a CanvasTexture. The last two are real GPU resources,
 * which is why identity churn showed up in the profile as browser-owned
 * object count rather than as bytes.
 *
 * None of that is what a badge is. A badge is a position, a size and a
 * texture, and this writes those into a slot that already exists. Sprites
 * and materials are created once and reused forever; the pool only ever
 * grows to the most that were on screen at once, and nothing is disposed on
 * the way to the next frame.
 *
 * It is also the layer that has to survive the move off three.js. A pool of
 * slots written by index is one instanced draw away from what WebGPU wants;
 * a React element per badge has no counterpart in linear memory at all,
 * which is the same reason the candidate arrays became lanes.
 *
 * React keeps exactly one job here: mounting `group` once. Everything
 * inside is written, not declared.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

import type { BillboardAtlas } from "./atlas.ts";
import { apply_anchor_occlusion } from "./anchor_occlusion.ts";
import { distance_fade_factor } from "./distance_fade.ts";
import { WORLD_UNITS_PER_PX } from "./canvas_scale.ts";

const $world = new THREE.Vector3();
const LANE = { init_cap: 256, alloc: CORE_ALLOC } as const;

export class BillboardField {
  readonly group = new THREE.Group();

  /** Live this frame. Slots past it are parked, not destroyed. */
  count: u32 = 0;

  private readonly sprites: THREE.Sprite[] = [];
  private readonly materials: THREE.SpriteMaterial[] = [];

  /** One window per atlas slot, at the slot's index. The window's rectangle
   * is rewritten on every push, so a slot the atlas reuses for another
   * badge is right the moment it is pushed. */
  private readonly views: (THREE.Texture | undefined)[] = [];
  private sheet: THREE.Texture | null = null;

  /** Local position of each slot's anchor, in `group`'s own frame. */
  private readonly x = new Vector(Float64Array, LANE);
  private readonly y = new Vector(Float64Array, LANE);
  private readonly z = new Vector(Float64Array, LANE);
  /** Full-strength drawn size in world units, and where the anchor falls
   * inside the sprite (0 top, 1 bottom) — see use_distance_fade's own
   * `center_frac`, which this replaces for the pooled case. */
  private readonly w = new Vector(Float64Array, LANE);
  private readonly h = new Vector(Float64Array, LANE);
  private readonly anchor_frac = new Vector(Float64Array, LANE);
  /** Which atlas slot each sprite shows, so a sheet that is replaced
   * mid-pass can be re-pointed for the sprites already written. */
  private readonly atlas_slot = new Vector(Uint32Array, LANE);

  constructor(private readonly render_order: f64) {
    this.group.name = "billboard-field";
    // Sprites carry their own world position; nothing here is culled as a
    // group, and a group-level bounding box would be recomputed every frame
    // for no gain.
    this.group.frustumCulled = false;
  }

  /** Opens a frame. Slots keep their contents until overwritten. */
  begin(): void {
    this.count = 0;
  }

  /**
   * Writes one badge into the next slot, growing the pool if this is the
   * most that have ever been live at once.
   *
   * The badge is `atlas_slot` of `atlas`. Assigning a slot's window to a
   * sprite that already had it is free; assigning a different one is a map
   * swap, not a program recompile, since a sprite always has some map from
   * the moment it is first used.
   */
  push(x: f64, y: f64, z: f64, atlas: BillboardAtlas, atlas_slot: u32, logical_w: f64, logical_h: f64, center_y: f64): void {
    const i = this.count++;
    if (i >= this.sprites.length) this.grow();

    this.x.set(i, x);
    this.y.set(i, y);
    this.z.set(i, z);
    this.w.set(i, logical_w * WORLD_UNITS_PER_PX);
    this.h.set(i, logical_h * WORLD_UNITS_PER_PX);
    this.anchor_frac.set(i, logical_h > 0 ? center_y / logical_h : 1);
    this.atlas_slot.set(i, atlas_slot);

    // A grown sheet is a different texture object over a different image, so
    // every window onto the old one is stale — including the ones already
    // handed to the sprites written earlier in this same pass, since a badge
    // drawn for a later sprite is what grew the sheet. Those sprites would
    // otherwise show a disposed texture until the next placement happened to
    // write them again, which on a still camera is never: a fresh load in a
    // dense area drew nothing until the first pan (2026-09-12).
    if (atlas.texture !== this.sheet) {
      for (const view of this.views) view?.dispose();
      this.views.length = 0;
      this.sheet = atlas.texture;
      for (let j = 0; j < i; j++) this.materials[j].map = this.sheet_view(atlas, this.atlas_slot.mem[j]);
    }

    const material = this.materials[i];
    const view = this.sheet_view(atlas, atlas_slot);
    if (material.map !== view) material.map = view;
  }

  /**
   * A window onto the shared sheet, one per rectangle.
   *
   * `clone()` copies the transform and shares the `Source`, which is what
   * three.js keys the GPU upload on — so five hundred of these are five
   * hundred sets of offset/repeat over one uploaded image. It is not the
   * end state: a sprite still carries its own material and so its own draw
   * call, and only an instanced quad with the rectangle as a vertex
   * attribute collapses those. This is the half that removes the memory and
   * the upload churn, ahead of the half that removes the draw calls.
   */
  private sheet_view(atlas: BillboardAtlas, slot: u32): THREE.Texture {
    let view = this.views[slot];
    if (!view) {
      view = atlas.texture.clone();
      this.views[slot] = view;
    }
    // The rectangle itself moves when the sheet repacks and the slot stays
    // (atlas.ts), so this is read every push rather than once.
    //
    // V is inverted here and nowhere else. A sprite's own v runs up from its
    // bottom edge while the sheet's runs down from the canvas's top, so the
    // offset is the entry's *lower* edge and the repeat is negative. The
    // sheet is uploaded unflipped precisely so this is one legible line
    // rather than an invisible transform applied to the whole image.
    const u0 = atlas.u0(slot), v0 = atlas.v0(slot), u1 = atlas.u1(slot), v1 = atlas.v1(slot);
    view.offset.set(u0, v1);
    view.repeat.set(u1 - u0, v0 - v1);
    return view;
  }

  /** Parks every slot past `count`. Cheaper than hiding them in `push`, and
   * it keeps the pool's high-water mark rather than shrinking to it. */
  end(): void {
    for (let i = this.count; i < this.sprites.length; i++) {
      if (!this.sprites[i].visible) break;
      this.sprites[i].visible = false;
    }
  }

  /**
   * Per-frame scale, opacity and anchor pinning, for every live slot.
   *
   * One loop where there used to be one useFrame subscription per badge. A
   * THREE.Sprite scales around its own centre, so pinning any other point
   * means recomputing position.z from the live scale each frame rather than
   * baking an offset in once — the same reason use_distance_fade does it,
   * and the reason this cannot be a static transform.
   */
  update(camera: THREE.Camera, viewport_h: f64): void {
    const fov = (camera as THREE.PerspectiveCamera).fov;
    const px_k = viewport_h / (2 * Math.tan((fov * Math.PI) / 360));
    this.group.updateMatrixWorld();

    for (let i = 0; i < this.count; i++) {
      const sprite = this.sprites[i];
      sprite.visible = true;

      $world.set(this.x.mem[i], this.y.mem[i], this.z.mem[i]).applyMatrix4(this.group.matrixWorld);
      const dist = $world.distanceTo(camera.position);
      const { scale, opacity } = distance_fade_factor(px_k / Math.max(1e-3, dist), dist);

      const h = this.h.mem[i] * scale;
      sprite.scale.set(this.w.mem[i] * scale, h, 1);
      // Both halves of one fact: how far the sprite's centre sits above its
      // anchor. The position keeps the anchor where it belongs while the
      // sprite rescales; the uniform lets anchor_occlusion.ts's shader
      // recover the anchor from the centre it is given.
      const drop = h * (this.anchor_frac.mem[i] - 0.5);
      sprite.position.set(this.x.mem[i], this.y.mem[i], this.z.mem[i] + drop);

      const material = this.materials[i];
      material.opacity = opacity;
      const uniform = material.userData?.anchor_drop as { value: number } | undefined;
      if (uniform) uniform.value = drop;
    }
  }

  /** Dev: what the pool holds and shows, for the ledger's own report. */
  stats(): { slots: u32; live: u32; visible: u32; mapped: u32; mounted: boolean } {
    let visible = 0, mapped = 0;
    for (let i = 0; i < this.sprites.length; i++) {
      if (this.sprites[i].visible) visible++;
      if (this.materials[i].map) mapped++;
    }
    // Mounted means reachable from a scene, not merely having a parent — a
    // frame group that was replaced would still be a parent.
    let root: THREE.Object3D | null = this.group;
    while (root.parent) root = root.parent;
    return { slots: this.sprites.length, live: this.count, visible, mapped, mounted: (root as THREE.Scene).isScene === true };
  }

  /** Releases every GPU resource this owns. Textures are the caller's. */
  dispose(): void {
    for (const view of this.views) view?.dispose();
    this.views.length = 0;
    for (const material of this.materials) material.dispose();
    this.materials.length = 0;
    this.sprites.length = 0;
    this.group.clear();
    this.count = 0;
  }

  private grow(): void {
    // One material per slot rather than one shared: opacity and the anchor
    // uniform are per badge, so they cannot share an instance. Built by hand
    // because the occlusion patch is an onBeforeCompile hook.
    const material = apply_anchor_occlusion(
      new THREE.SpriteMaterial({ transparent: true, depthTest: true, depthWrite: false }),
    );
    const sprite = new THREE.Sprite(material);
    sprite.renderOrder = this.render_order;
    // Depth is written by the occlusion shader's upright twin, not by the
    // sprite — see sprite_billboard.tsx on why a Sprite's own depth punches
    // a band through the middle of a badge.
    sprite.frustumCulled = false;
    sprite.visible = false;
    this.group.add(sprite);
    this.sprites.push(sprite);
    this.materials.push(material);
  }
}
