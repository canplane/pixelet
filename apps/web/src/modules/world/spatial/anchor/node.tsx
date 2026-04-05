// @pxl/web/modules/world/spatial/anchor/node.tsx

/**
 * Anchor-local scene node.
 * - resolves an L4 anchor group
 * - portals children into that anchor
 * - applies anchor-local PVF3 offset
 */

import { type ReactNode, useLayoutEffect, useState } from "react";
import * as THREE from "three";
import { createPortal } from "@react-three/fiber";

import { type FVec, type IVec, ivec, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L4_BASE, L4_IDX } from "@pxl/spec/space/indexer.ts";

import { useAnchorRegistry } from "./context.tsx";
import { AnchorRegistry } from "./registry.ts";

type AnchorNodeProps = {
  /** canonical PVF3 absolute coordinate */
  i: IVec;
  f?: FVec;

  children: ReactNode;
};
export function AnchorNode(props: AnchorNodeProps) {
  const { i, f = 0n, children } = props;

  const registry: AnchorRegistry = useAnchorRegistry();

  const [target, set_target] = useState<THREE.Group | null>(null);

  /** Resolve the L4 anchor that owns this node. */
  const anchor_idx: IVec = L4_IDX(i);

  /**
   * NOTE:
   * AnchorNode does not react to origin changes directly.
   * Floating-origin rebasing is handled at the anchor-group level.
   */
  useLayoutEffect(() => {
    const group: THREE.Group = registry.link(anchor_idx);
    set_target(group);

    return () => void registry.unlink(anchor_idx);
  }, [anchor_idx]);

  if (!target) return null;

  /** Anchor-local offset within the resolved L4 anchor. */
  const anchor: IVec = L4_BASE(i);
  const local: PVF3 = { i: ivec.sub(i, anchor), f };
  return createPortal(
    <group
      position={[
        pvf3.to_real(pvf3.X(local)),
        pvf3.to_real(pvf3.Y(local)),
        pvf3.to_real(pvf3.Z(local)),
      ]}
    >
      {children}
    </group>,
    target,
  );
}
