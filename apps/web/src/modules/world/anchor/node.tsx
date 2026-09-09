// @pxl/web/modules/world/anchor/node.tsx

/**
 * Anchor-local scene node.
 * - owns a holder group, attached to the L4 anchor that covers its coordinate
 * - portals children into that holder
 * - applies anchor-local PVF3 offset
 *
 * The holder exists from the first render on purpose. The obvious shape —
 * resolve the anchor group in a layout effect, hold it in state, `return null`
 * until it arrives — costs **two render passes for every AnchorNode**, and
 * children do not mount at all on the first. A DevTools capture of one
 * zoom-out counted 2121 AnchorNode / 2071 Portal mounts against 2892 Chunk
 * mounts, inside React scheduler runs of 2–5.6s; the double pass is a
 * straight multiplier on that. Owning the target instead means no state, no
 * null return, and one pass.
 *
 * Attaching the holder to the shared anchor group stays in a layout effect,
 * which is also where the registry refcount belongs — link and unlink are
 * then trivially balanced, and StrictMode's setup/cleanup/setup replay
 * detaches and reattaches the *same* holder rather than invalidating a portal
 * target mid-flight.
 */

import { type ReactNode, useLayoutEffect, useMemo } from "react";
import * as THREE from "three";
import { createPortal } from "@react-three/fiber";

import { type FVec, type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L4_BASE, L4_IDX } from "@pxl/shared/spec/space/indexer.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

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

  /** Resolve the L4 anchor that owns this node. */
  const anchor_idx: IVec = L4_IDX(i);

  /** This node's own group inside that anchor. Carries the anchor-local
   * offset directly rather than wrapping children in a second `<group>` —
   * one fewer R3F element per node, and there are thousands. */
  const holder = useMemo(() => new THREE.Group(), []);

  /** Anchor-local offset within the resolved L4 anchor. */
  const local_pos = useMemo((): V3 => {
    const anchor: IVec = L4_BASE(i);
    const local: PVF3 = { i: ivec.sub(i, anchor), f };
    return [pvf3.to_real(pvf3.X(local)), pvf3.to_real(pvf3.Y(local)), pvf3.to_real(pvf3.Z(local))];
  }, [i, f]);

  useLayoutEffect(() => {
    holder.position.set(local_pos[0], local_pos[1], local_pos[2]);
  }, [holder, local_pos]);

  /**
   * NOTE:
   * AnchorNode does not react to origin changes directly.
   * Floating-origin rebasing is handled at the anchor-group level.
   */
  useLayoutEffect(() => {
    const group: THREE.Group = registry.link(anchor_idx);
    group.add(holder);

    return () => {
      group.remove(holder);
      registry.unlink(anchor_idx);
    };
  }, [registry, anchor_idx, holder]);

  return createPortal(children, holder);
}
