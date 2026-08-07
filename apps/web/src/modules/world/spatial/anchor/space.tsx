// @pxl/web/modules/world/spatial/anchor/space.tsx

/**
 * Anchor scene-space coordinator.
 * - mounts the registry root into the scene
 * - provides the registry (owned by the caller, see World) to descendants
 *
 * Rebasing anchors on origin change used to happen in a useFrame here, as
 * its own independent subscription from use_frame_translation.ts's frame
 * snap — two separate per-frame callbacks reacting to the same map_origin
 * change with no ordering contract between them, which could leave anchors
 * and the local-world-frame disagreeing about the origin for a tick (seen
 * as a stray offset/doubled instance). The registry is now owned by World
 * and rebased from inside useFrameTranslation's own callback instead, so
 * it's one atomic step rather than two independently-scheduled ones.
 */

import type { ReactNode } from "react";

import type { AnchorRegistry } from "./registry.ts";
import { AnchorRegistryContext } from "./context.tsx";

type AnchorSpaceProps = {
  registry: AnchorRegistry;

  children: ReactNode;
};
export function AnchorSpace(props: AnchorSpaceProps) {
  const { registry, children } = props;

  return (
    <AnchorRegistryContext.Provider value={registry}>
      <primitive object={registry.root} />
      {children}
    </AnchorRegistryContext.Provider>
  );
}
