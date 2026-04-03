// @pxl/web/modules/world/spatial/anchor/space.tsx

/**
 * Anchor scene-space coordinator.
 * - owns the registry lifetime
 * - shifts live anchors on origin changes
 * - mounts the registry root into the scene
 */

import { type ReactNode, useEffect, useMemo } from "react";

import type { IVec } from "@pxl/spec/space/pvf3/mod.ts";

import { AnchorRegistry } from "./registry.ts";
import { AnchorRegistryContext } from "./context.tsx";

type AnchorSpaceProps = {
  map_origin: IVec;

  children: ReactNode;
};
export function AnchorSpace(props: AnchorSpaceProps) {
  const { map_origin, children } = props;

  /** Registry lifetime is owned by AnchorSpace. */
  const registry = useMemo(() => new AnchorRegistry(), []);

  /** Shift all live anchors when the floating origin changes. */
  useEffect(() => {
    registry.shift_map_origin(map_origin);
  }, [map_origin, registry]);

  return (
    <AnchorRegistryContext.Provider value={registry}>
      <primitive object={registry.root_group} />
      {children}
    </AnchorRegistryContext.Provider>
  );
}
