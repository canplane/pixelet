// @pxl/web/modules/world/markers/cell-highlighter/bridge.tsx

import { useLayoutEffect, useRef } from "react";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useAnchorRegistry } from "../../spatial/anchor/context.tsx";
import { CellHighlighterObject } from "./object.ts";

// Highlighter style
export function CellHighlighter() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);

  const registry = useAnchorRegistry();
  const obj_ref = useRef<CellHighlighterObject | null>(null);

  useLayoutEffect(() => {
    const obj = new CellHighlighterObject(registry);
    obj_ref.current = obj;

    return () => {
      obj.dispose();
      obj_ref.current = null;
    };
  }, [registry]);

  useLayoutEffect(() => {
    obj_ref.current?.set_coord(hovered);
  }, [hovered]);

  return null;
}
