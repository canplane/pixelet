// @pxl/web/modules/world/terrain/layer.tsx

import { memo, useMemo } from "react";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

import type { Terrain } from "@pxl/shared/bake/format.ts";
import { page_to_chunks } from "@/render/terrain/chunk.ts";

import { MapStatus, useMapStore } from "@/modules/map/store.ts";
import { useTileStore } from "./tile_store.ts";
import { CHEBYSHEV_DIST } from "./use_tile_fetcher.ts";
import { RENDER_RADIUS } from "./config.ts";
import { AnchorNode } from "@/modules/world/spatial/anchor/node.tsx";

import { TerrainChunk } from "./chunk.tsx";

type TerrainPageProps = {
  page_idx: IVec;
  page: Terrain;
};
const TerrainPage = memo((props: TerrainPageProps) => {
  const { page_idx, page } = props;
  const { ground, ele } = page;

  const page_base: IVec = FROM_L3_IDX(page_idx);
  const chunks: [IVec, Terrain][] = useMemo(() => page_to_chunks(ground, ele), [ground, ele]);

  return (
    <AnchorNode i={page_base}>
      <group name={`terrain-page/${ivec.repr(page_idx)}`}>
        {chunks.map(([page_off, chunk]) => (
          <TerrainChunk
            key={page_off.toString()}
            page_idx={page_idx}
            page_off={page_off}
            chunk={chunk}
          />
        ))}
      </group>
    </AnchorNode>
  );
});
TerrainPage.displayName = "TerrainPage";

export const TerrainLayer = memo(() => {
  const map_status = useMapStore((state) => state.map_status);
  const map_center = useMapStore((state) => state.map_center);
  const tile_map = useTileStore((state) => state.tile_map);

  const tiles = useMemo(() => Array.from(tile_map.entries()), [tile_map]);

  // RENDER_RADIUS (currently == CLEANUP_RADIUS, see config.ts) bounds what
  // actually becomes a scene object — separate from LOAD_RADIUS, which
  // only bounds what use_tile_fetcher.ts guarantees is fetched. Confirmed
  // live: mounting everything already cached (rather than the much
  // tighter LOAD_RADIUS box) reads as smooth, fog-hidden falloff instead
  // of a hard pop at the LOAD_RADIUS edge, and the LOD system's own
  // coarse-instance falloff keeps that from being as expensive as
  // rendering that page count at full detail would be. get_ele()
  // (use_elevation.ts, e.g. location_puck's own device-page lookup) reads
  // tile_map directly and doesn't need a page mounted here to resolve.
  const center_page_idx: IVec | null = useMemo(() => {
    if (map_center === null) return null;
    return L3_IDX(map_center.i);
  }, [map_center]);

  const visible_tiles = useMemo(() => {
    if (center_page_idx === null) return tiles;
    return tiles.filter(([page_idx]) => CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx)) <= RENDER_RADIUS);
  }, [tiles, center_page_idx]);

  // hooks above must run every render (rules of hooks) — this early return
  // has to stay below all of them, not before.
  if (tiles.length === 0 && map_status === MapStatus.LOADING) return null;

  return (
    <group name="terrain-layer">
      {visible_tiles.map(([page_idx, tile]) => (
        <TerrainPage
          key={page_idx.toString()}
          page_idx={page_idx}
          page={tile.terrain}
        />
      ))}
    </group>
  );
});
TerrainLayer.displayName = "TerrainLayer";
