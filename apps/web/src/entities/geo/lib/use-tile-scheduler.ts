import { useEffect, useRef, useCallback } from 'react';
import { useMapStore } from '@/entities/geo/model/map-store';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { tileWorkerManager } from '@/entities/geo/lib/workers/tile-worker-manager';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
// 🎯 [Refactor] tile-utils 최신 스펙 반영 (TileCoord, x, y)
import {
  getTileCoord,
  getTileKey,
  type TileCoord,
} from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { useTileStore } from '@/entities/geo/model/tile-store';

const { LOAD_RADIUS, CLEANUP_RADIUS } = GEO_CONSTANTS;

interface TileRequest {
  key: string;
  coord: TileCoord;
  distance: number;
}

export function useTileScheduler() {
  const { center } = useMapNavigation();

  const setStatus = useMapStore((state) => state.setStatus);

  // TileStore에서 필요한 상태와 액션들을 미리 구독
  const tilesMap = useTileStore((state) => state.tilesMap);
  const setTile = useTileStore((state) => state.setTile);
  const clearOldTiles = useTileStore((state) => state.clearOldTiles);

  const lastCenterTileKey = useRef<string>('');
  const processingTiles = useRef<Set<string>>(new Set());
  const activeBatchId = useRef<number>(0);

  /**
   * [1] Scheduling Queue Generator
   */
  const getScheduledQueue = useCallback(
    (centerCoord: TileCoord): TileRequest[] => {
      const pendingRequests: TileRequest[] = [];
      const { tx, ty } = centerCoord;

      for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
        for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
          const targetCoord = { tx: tx + dx, ty: ty + dy };
          const key = getTileKey(targetCoord);

          if (!tilesMap.has(key) && !processingTiles.current.has(key)) {
            const distance = dx * dx + dy * dy;
            pendingRequests.push({ key, coord: targetCoord, distance });
          }
        }
      }
      return pendingRequests.sort((a, b) => a.distance - b.distance);
    },
    [tilesMap], // 🎯 의존성에 tilesMap 추가
  );

  /**
   * [2] 범위 밖 타일 정리 (메모리 관리 / Garbage Collection)
   */
  const cleanupTiles = useCallback(
    (neededKeys: string[]) => {
      // 🎯 위에서 뽑아둔 액션을 그대로 사용
      clearOldTiles(neededKeys);
    },
    [clearOldTiles],
  );

  /**
   * [3] 개별 타일 비동기 로딩 및 점진적 업데이트 실행 (Async Loader)
   */
  const loadTile = useCallback(
    async (request: TileRequest) => {
      const { key, coord } = request;
      processingTiles.current.add(key);

      try {
        const finalTile = await tileWorkerManager.requestTile(
          key,
          coord,
          // [점진적 업데이트] Worker가 중간 결과를 보낼 때마다 실행됨
          (partialTile) => setTile(key, partialTile),
        );

        // [최종 완료] 최종 결과물이 유효하면 스토어 업데이트
        if (finalTile && finalTile.terrain) {
          setTile(key, finalTile);
          console.log(`[Tile Scheduler] ✅ Loaded Tile (${key})`);
        }
      } catch (err) {
        console.error(`[Tile Scheduler] ❌ Worker Error (${key}):`, err);
      } finally {
        processingTiles.current.delete(key);
      }
    },
    [setTile],
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

    cleanupTiles([...activeKeys]);

    const queue = getScheduledQueue(centerCoord);

    if (queue.length > 0) {
      setStatus('loading');
      const tasks = queue.map((req) => loadTile(req));

      Promise.all(tasks).finally(() => {
        if (batchId === activeBatchId.current) {
          setStatus('success');
        }
      });
    } else {
      const { tilesMap: currentTilesMap } = useTileStore.getState();
      if (currentTilesMap.size > 0) setStatus('success');
    }
  }, [center, setStatus, cleanupTiles, getScheduledQueue, loadTile]);
}
