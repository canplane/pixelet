// @pxl/web/modules/world/spatial/anchor/context.tsx

/**
 * Anchor React wiring.
 * - provides AnchorRegistry through React context
 * - exposes useAnchorRegistry()
 */

import { createContext, useContext } from "react";
import type { AnchorRegistry } from "./registry.ts";

export const AnchorRegistryContext = createContext<AnchorRegistry | null>(null);

/** Resolve the current AnchorRegistry from context. */
export function useAnchorRegistry(): AnchorRegistry {
  const ctx = useContext(AnchorRegistryContext);
  if (!ctx) throw new Error("useAnchorRegistry must be used within an AnchorSpace");
  return ctx;
}
