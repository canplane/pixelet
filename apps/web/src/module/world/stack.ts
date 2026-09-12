// @pxl/web/module/world/stack.ts

/**
 * The registered WorldLayer stack — module-level, not per-render, since
 * WorldLayer.paint() output changes per call, never the registered layer
 * object's own identity. Shared (not defined inline in world.tsx)
 * because camera/ray.ts needs the exact same stack to composite the same
 * chunks it raycasts against — one registration, both consumers.
 */

import { TerrainWorldLayer } from "@/render/layer/terrain/adapter.ts";
import { ObjectWorldLayer } from "@/render/layer/object/adapter.ts";
import type { WorldLayerStack } from "@/render/layer/pipeline/contract.ts";

export const CHUNK_STAGE_LAYERS: WorldLayerStack = [new TerrainWorldLayer(), new ObjectWorldLayer()];
