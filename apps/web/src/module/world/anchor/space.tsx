// @pxl/web/module/world/anchor/space.tsx

/**
 * Anchor scene-space coordinator.
 * - mounts the registry root into the scene
 * - provides the registry (owned by the caller, see World) to descendants
 *
 * The registry is rebased from inside useFrameTranslation's own callback
 * rather than its own useFrame here, so origin rebasing and the frame snap
 * happen as one atomic step instead of two independently-scheduled per-frame
 * callbacks that could leave anchors and the local-world-frame disagreeing
 * about the origin for a tick (a stray offset/doubled instance).
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
