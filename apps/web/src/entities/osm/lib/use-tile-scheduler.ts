import { useEffect, useRef, useCallback } from 'react';
import { useMapStore } from '@/entities/osm/model/map-store';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { tileWorkerManager } from '@/entities/osm/lib/workers/tile-worker-manager';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
// 🎯 [Refactor] tile-utils 최신 스펙 반영 (TileCoord, x, y)
import {
  getTileCoord,
  type TileCoord,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

const { DEFAULT_RENDER_RADIUS, CLEANUP_RADIUS } = OSM_CONSTANTS;

interface TileRequest {
  coord: TileCoord;
  // key: string; -> 🗑️ 삭제됨 (중복 데이터)
  distance: number;
}

/**
 * [Helper] 좌표를 스토어/캐시용 키 문자열로 변환 (Derived State)
 * 필요할 때마다 즉시 생성해서 씁니다.
 */
const getTileKey = (coord: TileCoord) => `${coord.tx}:${coord.ty}`;

export function useTileScheduler() {
  const { center } = useMapNavigation();

  const setStatus = useMapStore((state) => state.setStatus);
  const setGeometryTile = useMapStore((state) => state.setGeometryTile);
  const removeGeometryTile = useMapStore((state) => state.removeGeometryTile);

  const lastCenterTileKey = useRef<string>('');
  const processingTiles = useRef<Set<string>>(new Set());
  const activeBatchId = useRef<number>(0);

  /**
   * [1] Scheduling Queue Generator
   */
  const getScheduledQueue = useCallback(
    (centerCoord: TileCoord): TileRequest[] => {
      const pendingRequests: TileRequest[] = [];
      const { tiles } = useMapStore.getState();
      const { tx, ty } = centerCoord;

      for (let dx = -DEFAULT_RENDER_RADIUS; dx <= DEFAULT_RENDER_RADIUS; dx++) {
        for (
          let dy = -DEFAULT_RENDER_RADIUS;
          dy <= DEFAULT_RENDER_RADIUS;
          dy++
        ) {
          const targetCoord = { tx: tx + dx, ty: ty + dy };
          const key = getTileKey(targetCoord); // 🎯 존재 여부 확인용으로만 생성

          // 이미 로드되었거나 로딩 중이면 스킵
          if (!tiles.has(key) && !processingTiles.current.has(key)) {
            const distance = dx * dx + dy * dy;
            // 🎯 Queue에는 순수 좌표 데이터만 넣습니다.
            pendingRequests.push({
              coord: targetCoord,
              distance,
            });
          }
        }
      }

      return pendingRequests.sort((a, b) => a.distance - b.distance);
    },
    [],
  );

  /**
   * [2] Garbage Collection
   */
  const cleanupTiles = useCallback(
    (activeKeys: Set<string>) => {
      const { tiles } = useMapStore.getState();
      tiles.forEach((_, key) => {
        if (!activeKeys.has(key) && !processingTiles.current.has(key)) {
          removeGeometryTile(key);
        }
      });
    },
    [removeGeometryTile],
  );

  /**
   * [3] Async Loader
   * 🎯 Key 인자를 제거하고 Coord만 받습니다.
   */
  const loadTile = useCallback(
    async (coord: TileCoord) => {
      const key = getTileKey(coord); // 🎯 내부 식별용으로 즉시 생성
      processingTiles.current.add(key);

      try {
        const finalTile = await tileWorkerManager.requestTile(
          coord,
          // Progressive Update
          (partialTile) => {
            setGeometryTile(coord, partialTile);
          },
        );

        // Final Update
        if (finalTile && finalTile.grids) {
          setGeometryTile(coord, finalTile);
        }
      } catch (err) {
        console.error(`[TileScheduler] ❌ Worker Error (${key}):`, err);
      } finally {
        processingTiles.current.delete(key);
      }
    },
    [setGeometryTile],
  );

  // [Main Loop]
  useEffect(() => {
    if (!center) return;

    const centerCoord = getTileCoord(center);
    const currentKey = getTileKey(centerCoord);

    if (lastCenterTileKey.current === currentKey) return;
    lastCenterTileKey.current = currentKey;

    const batchId = ++activeBatchId.current;

    // Cleanup Scope (Set<string>은 여전히 Key 기반으로 관리하는 것이 효율적)
    const activeKeys = new Set<string>();
    const { tx, ty } = centerCoord;
    for (let dx = -CLEANUP_RADIUS; dx <= CLEANUP_RADIUS; dx++) {
      for (let dy = -CLEANUP_RADIUS; dy <= CLEANUP_RADIUS; dy++) {
        activeKeys.add(getTileKey({ tx: tx + dx, ty: ty + dy }));
      }
    }

    cleanupTiles(activeKeys);

    const queue = getScheduledQueue(centerCoord);

    if (queue.length > 0) {
      setStatus('loading');
      // 🎯 인자가 coord 하나로 줄어들어 깔끔해짐
      const tasks = queue.map((req) => loadTile(req.coord));

      Promise.all(tasks).finally(() => {
        if (batchId === activeBatchId.current) {
          setStatus('success');
        }
      });
    } else {
      const { tiles: currentTiles } = useMapStore.getState();
      if (currentTiles.size > 0) setStatus('success');
    }
  }, [center, setStatus, cleanupTiles, getScheduledQueue, loadTile]);
}
