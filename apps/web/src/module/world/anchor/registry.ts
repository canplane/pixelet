// @pxl/web/module/world/anchor/registry.ts

/**
 * AnchorRegistry
 *
 * Live anchor-group registry for the current scene.
 * - owns anchor root groups
 * - ref-counts active anchor users
 * - shifts live anchors when floating origin changes
 */

import * as THREE from "three";

import type { u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L4_IDX } from "@pxl/shared/spec/space/indexer.ts";

import { FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

/**
 * The live anchors as a table: the L4 index in an IndexedMap in the core
 * memory, the link count in a lane at its position, and the group — the
 * scene object every AnchorNode under that anchor hangs from, the one part
 * three.js has to own — as the handle at the same position.
 */
export class AnchorRegistry {
  /** Root group that owns all live L4 anchor groups. */
  readonly root = new THREE.Group();

  /** Current local-world origin used to place live anchor groups. */
  cur_map_origin: IVec = 0n;

  private readonly index = new IndexedMap<bigint>(I64_KEYS, 64, FIXED_ALLOC);
  private readonly nlink = new Vector(Uint32Array, { init_cap: 64, alloc: FIXED_ALLOC });
  private readonly groups: THREE.Group[] = [];

  /** Live anchors. */
  get size(): u32 {
    return this.index.size;
  }

  /** Acquire the anchor group for the given L4 anchor index. */
  link(anchor_idx: IVec): THREE.Group {
    let pos = this.index.find(anchor_idx);
    if (pos < 0) {
      const group = new THREE.Group();
      group.name = ivec.repr(anchor_idx);
      this._update_group_pos(group, anchor_idx);
      this.root.add(group);
      pos = this.index.insert(anchor_idx);
      this.nlink.set(pos, 0);
      this.groups[pos] = group;
    }
    this.nlink.mem[pos]++;
    return this.groups[pos];
  }

  /** Release one reference to the given L4 anchor group. */
  unlink(anchor_idx: IVec): void {
    const pos = this.index.find(anchor_idx);
    if (pos < 0) return;
    if (--this.nlink.mem[pos] > 0) return;

    this.root.remove(this.groups[pos]);
    const from = this.index.remove_at(pos);
    if (from !== pos) {
      this.nlink.mem[pos] = this.nlink.mem[from];
      this.groups[pos] = this.groups[from];
    }
    this.nlink.truncate(from);
    this.groups.length = from;
  }

  /** Shift all live anchor groups to a new floating origin. */
  shift_map_origin(to: IVec): void {
    this.cur_map_origin = to;
    for (let pos = 0; pos < this.index.size; pos++) {
      this._update_group_pos(this.groups[pos], BigInt.asUintN(64, this.index.key_at(pos)));
    }
  }

  /** Update one anchor group's position relative to the current origin. */
  private _update_group_pos(group: THREE.Group, anchor_idx: IVec): void {
    const anchor_base: IVec = FROM_L4_IDX(anchor_idx);
    const local: IVec = ivec.sub(anchor_base, this.cur_map_origin);

    group.position.set(
      ivec.to_real(ivec.X(local)),
      ivec.to_real(ivec.Y(local)),
      ivec.to_real(ivec.Z(local)),
    );
    group.updateMatrix();
  }
}
