// @pxl/web/modules/world/markers/hover_cursor.tsx

import { useMemo } from "react";
import * as THREE from "three";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { center3n, fill3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { get_local_world_origin } from "../frame/coord.ts";
import { find_overlapping_object } from "../objects/collision.ts";
import { usePlacedObject } from "../objects/store.ts";
import { object_color_hex } from "@pxl/shared/spec/cell/objects.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { marker_colors } from "./style.ts";

// Flat, no border: a bordered box only reads cleanly on a primitive with
// per-face UVs mapped 0-1 (this one's own previous implementation) — an
// arbitrary decor mesh's own UV layout has nothing to do with its
// silhouette, so the same trick breaks on anything that isn't a box. A
// screen-space silhouette outline would generalize instead, but that's a
// real post-processing pass, not a per-mesh shader — tracked separately,
// not worth it just for this.
const GHOST_OPACITY = 0.5;

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

const TERRAIN_POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

export function HoverCursor() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);
  const hovered_object_id = useCameraStore((state) => state.hovered_object_id);
  const activeTool = useUiStore((state) => state.activeTool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const map_center = useMapStore((state) => state.map_center);

  const hovered_object = usePlacedObject(hovered_object_id);

  const use_object_style = activeTool === "object" || activeTool === "eraser" || hovered_object !== null;

  const preview_size = (activeTool === "eraser" || activeTool === "pointer") && hovered_object ? hovered_object.size : object_size;

  const object_pos = useMemo(() => center3n(fill3(0), preview_size), [preview_size]);

  const blocked = useMemo(() => {
    if (activeTool === "object") {
      if (!hovered) return false;
      const map_origin = get_local_world_origin(map_center);
      return find_overlapping_object(hovered, object_size, map_origin) !== null;
    }
    if (activeTool === "eraser") return hovered_object === null;
    return false;
  }, [activeTool, hovered, map_center, object_size, hovered_object]);

  // The real target ghosts at its own true color, whether it's an existing
  // object under the cursor (eraser or pointer alike — both are "here's the
  // real thing", not "here's what this tool does") or a placement
  // candidate. Bare terrain with no object and no placement in flight has
  // no real color to borrow, so it keeps the generic marker color.
  const color: string = hovered_object ? object_color_hex(hovered_object.color_id) : activeTool === "object" ? object_color_hex(object_color_id) : marker_colors.ACTION;

  const current_scale = use_object_style ? preview_size : U_PER_CELL;

  if (!hovered || blocked) return null;
  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={use_object_style ? object_pos : TERRAIN_POS}>
        <mesh
          geometry={HIGHLIGHT_BOX}
          scale={current_scale}
          // 9999 pairs with the terrain case's strong polygon offset below;
          // object mode uses the default order so real depth occlusion works.
          renderOrder={use_object_style ? 0 : 9999}
        >
          <meshBasicMaterial
            color={color}
            transparent
            opacity={GHOST_OPACITY}
            side={THREE.DoubleSide}
            depthTest
            polygonOffset
            // Object mode needs only a small polygon-offset nudge (it's not
            // coplanar with a distant surface like the terrain case is) —
            // reusing the terrain case's strong bias here would also push
            // the cursor in front of a placed cube that should legitimately
            // occlude it.
            polygonOffsetFactor={use_object_style ? -1 : -2}
            polygonOffsetUnits={use_object_style ? -1 : -8}
          />
        </mesh>
      </group>
    </AnchorNode>
  );
}
