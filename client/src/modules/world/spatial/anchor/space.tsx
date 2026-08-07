// @pxl/web/modules/world/spatial/anchor/space.tsx

/**
 * Anchor scene-space coordinator.
 * - owns the registry lifetime
 * - shifts live anchors on origin changes
 * - mounts the registry root into the scene
 */

import { type ReactNode, useMemo } from "react";
import { useFrame } from "@react-three/fiber";

import type { IVec } from "@pixelet/shared/spec/space/pvf3/mod.ts";

import { AnchorRegistry } from "./registry.ts";
import { AnchorRegistryContext } from "./context.tsx";

type AnchorSpaceProps = {
  map_origin: IVec;

  children: ReactNode;
};
export function AnchorSpace(props: AnchorSpaceProps) {
  const { map_origin, children } = props;

  /** Registry lifetime is owned by AnchorSpace. */
  const registry = useMemo(() => {
    const next = new AnchorRegistry();
    next.cur_map_origin = map_origin;
    return next;
  }, []);

  /**
   * Shift all live anchors when the floating origin changes.
   * Runs on the R3F render tick (not a React effect) so it stays synchronized with
   * use-frame-translation.ts's frame snap — an effect-based shift can lag a tick behind,
   * leaving newly-linked anchors baked in with a stale origin until the next rebase.
   */
  useFrame(() => {
    if (registry.cur_map_origin !== map_origin) {
      registry.shift_map_origin(map_origin);
    }
  });

  return (
    <AnchorRegistryContext.Provider value={registry}>
      <primitive object={registry.root} />
      {children}
    </AnchorRegistryContext.Provider>
  );
}
