// @pxl/web/modules/world/terrain/layer.tsx

import { memo, useMemo } from "react";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

import type { Terrain } from "@pxl/shared/bake/format.ts";
import { page_to_chunks } from "@/render/terrain/chunk.ts";

import { MapStatus, useMapStore } from "@/modules/map/store.ts";
import { useTileStore } from "./tile_store.ts";
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
  const tile_map = useTileStore((state) => state.tile_map);

  const tiles = useMemo(() => Array.from(tile_map.entries()), [tile_map]);
  if (tiles.length === 0 && map_status === MapStatus.LOADING) return null;

  return (
    <group name="terrain-layer">
      {tiles.map(([page_idx, tile]) => (
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
