import { useEffect, useRef, useCallback } from 'react';
import { useMapStore } from '@/entities/geo/model/map-store';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { tileWorkerManager } from '@/entities/geo/lib/workers/tile-worker-manager';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
// 🎯 [Refactor] tile-utils 최신 스펙 반영 (TileCoord, x, y)
import { getTileCoord, getTileKey, type TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { useTileStore } from '@/entities/geo/model/tile-store';

const { LOAD_RADIUS } = GEO_CONSTANTS;
// 🎯 [Rule] Math.round(LOAD_RADIUS * 1.5) 공식을 통한 CLEANUP_RADIUS 확정
const CLEANUP_RADIUS = Math.round(LOAD_RADIUS * 1.5);

interface TileRequest {
  key: string;
  coord: TileCoord;
  distance: number;
}

export function useTileScheduler() {
  const { center } = useMapNavigation();

  const setStatus = useMapStore((state) => state.setStatus);

  // TileStore에서 필요한 상태와 액션들을 미리 구독
  const getTile = useTileStore((state) => state.getTile);
  const getTileMap = useTileStore((state) => state.tileMap); // Map 객체 직접 참조
  const setTile = useTileStore((state) => state.setTile);
  const clearTile = useTileStore((state) => state.clearTile);
  const unloadBuildLayer = useTileStore((state) => state.unloadBuildLayer);

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

          if (!getTile(key) && !processingTiles.current.has(key)) {
            // Chebyshev distance로 변경하여 정사각 그리드 일치
            const distance = Math.max(Math.abs(dx), Math.abs(dy));
            pendingRequests.push({ key, coord: targetCoord, distance });
          }
        }
      }
      return pendingRequests.sort((a, b) => a.distance - b.distance);
    },
    [getTile],
  );

  /**
   * [2] 범위 밖 타일 정리 (Soft/Hard Unload Strategy)
   * 🎯 [Optimization] [...tileMap] 순회를 통해 역산(getCoordFromKey) 없이 즉시 처리
   */
  const cleanupTiles = useCallback(
    (centerCoord: TileCoord) => {
      const tileMap = getTileMap; // zustand state에서 직접 가져온 Map

      // 🎯 [Reverse Parse Zero] Map Iterator를 사용하여 key와 tile.coord를 동시에 확보
      for (const [key, tile] of tileMap) {
        const { tx, ty } = tile.coord;
        const dx = Math.abs(tx - centerCoord.tx);
        const dy = Math.abs(ty - centerCoord.ty);
        const dist = Math.max(dx, dy);

        if (dist > CLEANUP_RADIUS) {
          // 🚀 [Hard Delete] 1.5배 범위 밖: 완전 제거
          clearTile(key);
        } else if (dist > LOAD_RADIUS) {
          // 🚀 [Soft Unload] h * h * (h/2) 전략: 3D 청크 데이터만 선택적 제거
          unloadBuildLayer(key);
        }
      }
    },
    [getTileMap, clearTile, unloadBuildLayer],
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
        if (finalTile) {
          setTile(key, finalTile);
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

    // 🎯 [Optimization] 로드된 타일 전수조사 방식으로 Cleanup 실행
    cleanupTiles(centerCoord);

    const queue = getScheduledQueue(centerCoord);

    if (queue.length > 0) {
      setStatus('loading');

      const groups: Record<number, TileRequest[]> = {};
      queue.forEach((req) => {
        if (!groups[req.distance]) groups[req.distance] = [];
        groups[req.distance].push(req);
      });

      const sortedDistances = Object.keys(groups)
        .map(Number)
        .sort((a, b) => a - b);

      // 🚀 동심원 로딩 시퀀스 실행
      (async () => {
        for (const dist of sortedDistances) {
          if (batchId !== activeBatchId.current) return;

          // 같은 거리에 있는 타일들은 병렬로 처리
          await Promise.all(groups[dist].map((req) => loadTile(req)));
        }

        if (batchId === activeBatchId.current) {
          setStatus('success');
        }
      })();
    }
  }, [center, setStatus, cleanupTiles, getScheduledQueue, loadTile]);
}
