// @pxl/web/modules/world/spatial/anchor/registry.ts

/**
 * AnchorRegistry
 *
 * Live anchor-group registry for the current scene.
 * - owns anchor root groups
 * - ref-counts active anchor users
 * - shifts live anchors when floating origin changes
 */

import * as THREE from "three";

import type { u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";
import { to3 } from "@pxl/spec/space/adapter/three.ts";
import { FROM_L4_IDX, L4_BASE } from "@pxl/spec/space/indexer.ts";

type Entry = {
  group: THREE.Group;
  nlink: u32;
};

export class AnchorRegistry {
  /** Root group that owns all live L4 anchor groups. */
  readonly root_group = new THREE.Group();

  /** Keyed by L4 anchor index. */
  readonly entries = new Map<IVec, Entry>();

  /** Acquire the anchor group for the given L4 base. */
  link(anchor_idx: IVec, origin: IVec): THREE.Group {
    let entry: Entry | undefined = this.entries.get(anchor_idx);

    if (!entry) {
      const group = new THREE.Group();
      group.name = ivec.repr(anchor_idx);

      this._update_group_pos(group, anchor_idx, origin);

      this.root_group.add(group);
      entry = { group, nlink: 0 };
      this.entries.set(anchor_idx, entry);
    }

    entry.nlink += 1;
    return entry.group;
  }

  /** Release one reference to the given L4 anchor group. */
  unlink(anchor_base: IVec): void {
    const entry: Entry | undefined = this.entries.get(anchor_base);
    if (!entry) return;

    entry.nlink -= 1;

    if (entry.nlink === 0) {
      this.root_group.remove(entry.group);
      this.entries.delete(anchor_base);
    }
  }

  /** Shift all live anchor groups to a new floating origin. */
  shift_map_origin(new_map_origin: IVec): void {
    for (const [anchor_idx, entry] of this.entries) {
      this._update_group_pos(entry.group, anchor_idx, new_map_origin);
    }
  }

  /** Update one anchor group's position relative to the current origin. */
  private _update_group_pos(group: THREE.Group, anchor_idx: IVec, new_map_origin: IVec): void {
    const anchor: IVec = L4_BASE(FROM_L4_IDX(anchor_idx));
    const local: IVec = ivec.sub(anchor, new_map_origin);

    group.position.set(...to3([
      ivec.to_real(ivec.X(local)),
      ivec.to_real(ivec.Y(local)),
      ivec.to_real(ivec.Z(local)),
    ]));
    group.updateMatrix();
  }
}
