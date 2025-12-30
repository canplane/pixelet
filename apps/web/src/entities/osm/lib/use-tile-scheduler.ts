import { useEffect, useRef, useCallback } from 'react';
import { useMapStore } from '@/entities/osm/model/map-store';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { tileWorkerManager } from '@/entities/osm/lib/workers/tile-worker-manager';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { getTileIndex } from '@pixelet/shared/entities/osm/lib/tile-math';

const { DEFAULT_RENDER_RADIUS, CLEANUP_RADIUS } = OSM_CONSTANTS;

interface TileRequest {
  tx: number;
  ty: number;
  key: string;
  distance: number;
}

/**
 * [use-tile-scheduler]
 * Web Worker를 통해 백그라운드에서 타일을 로드하고,
 * 점진적 렌더링(Progressive Rendering) 결과를 UI에 반영합니다.
 *
 * [Scheduling]
 * 중심 좌표를 기준으로 가까운 타일(Radius Sort)부터 우선적으로 Worker에 요청하여,
 * 사용자의 시야 중심부(Ripple Effect)가 가장 먼저 로딩되도록 스케줄링합니다.
 */
export function useTileScheduler() {
  const { center } = useMapNavigation();

  const setStatus = useMapStore((state) => state.setStatus);
  const setGeometryTile = useMapStore((state) => state.setGeometryTile);
  const removeGeometryTile = useMapStore((state) => state.removeGeometryTile);

  const lastCenterTileKey = useRef<string>('');
  const processingTiles = useRef<Set<string>>(new Set());
  // 🎯 [race-condition-guard] 현재 실행 중인 요청 배치의 유효성을 검증하기 위한 ID
  const activeBatchId = useRef<number>(0);

  /**
   * [1] 가시 범위 순회 및 로드 대상 후보군 수집
   */
  const getScheduledQueue = useCallback(
    (tx: number, ty: number): TileRequest[] => {
      const pendingRequests: TileRequest[] = [];
      const { tiles } = useMapStore.getState();

      for (let dx = -DEFAULT_RENDER_RADIUS; dx <= DEFAULT_RENDER_RADIUS; dx++) {
        for (
          let dy = -DEFAULT_RENDER_RADIUS;
          dy <= DEFAULT_RENDER_RADIUS;
          dy++
        ) {
          const targetTx = tx + dx;
          const targetTy = ty + dy;
          const key = `${targetTx}:${targetTy}`;

          // 3. 로드 요청 대기열 추가 (이미 로드된 타일 제외)
          if (!tiles.has(key) && !processingTiles.current.has(key)) {
            // [Priority Scheduling] 중심부와의 거리(가중치) 계산 (유클리드 거리 제곱)
            const distance = dx * dx + dy * dy;
            pendingRequests.push({ tx: targetTx, ty: targetTy, key, distance });
          }
        }
      }

      // 4. [핵심] 거리순 정렬 (가까운 타일 -> 먼 타일)
      // Worker는 FIFO(선입선출)로 메시지를 처리하므로, 요청 순서가 곧 로딩 순서가 됩니다.
      return pendingRequests.sort((a, b) => a.distance - b.distance);
    },
    [],
  );

  /**
   * [2] 범위 밖 타일 정리 (메모리 관리 / Garbage Collection)
   */
  const cleanupTiles = useCallback(
    (activeKeys: Set<string>) => {
      const { tiles } = useMapStore.getState();
      tiles.forEach((_, key) => {
        // 현재 보고 있는 범위(activeKeys)에도 없고, 로딩 중도 아니라면 삭제
        if (!activeKeys.has(key) && !processingTiles.current.has(key)) {
          removeGeometryTile(key);
        }
      });
    },
    [removeGeometryTile],
  );

  /**
   * [3] 개별 타일 비동기 로딩 및 점진적 업데이트 실행
   */
  const loadTile = useCallback(
    async (tx: number, ty: number, key: string) => {
      processingTiles.current.add(key);

      try {
        // [변경 2] tileWorkerManager를 통해 요청합니다.
        const finalTile = await tileWorkerManager.requestTile(
          tx,
          ty,
          // [점진적 업데이트] Worker가 중간 결과를 보낼 때마다 실행됨
          (partialTile) => {
            setGeometryTile(key, partialTile);
          },
        );

        // [최종 완료] 최종 결과물이 유효하면 스토어 업데이트
        if (finalTile && finalTile.grids) {
          setGeometryTile(key, finalTile);
        }
      } catch (err) {
        console.error(`[Lifecycle] Worker error ${key}:`, err);
      } finally {
        processingTiles.current.delete(key);
      }
    },
    [setGeometryTile],
  );

  useEffect(() => {
    if (!center) return;

    // 1. 현재 중심 타일 계산
    const { tx, ty } = getTileIndex(center);
    const currentKey = `${tx}:${ty}`;

    if (lastCenterTileKey.current === currentKey) return;
    lastCenterTileKey.current = currentKey;

    // 새로운 로딩 배치 생성
    const batchId = ++activeBatchId.current;

    const activeKeys = new Set<string>();
    // 삭제를 위한 더 넓은 범위의 키 집합 생성
    for (let dx = -CLEANUP_RADIUS; dx <= CLEANUP_RADIUS; dx++) {
      for (let dy = -CLEANUP_RADIUS; dy <= CLEANUP_RADIUS; dy++) {
        activeKeys.add(`${tx + dx}:${ty + dy}`);
      }
    }

    // 6. 범위 밖 타일 정리 (메모리 관리)
    cleanupTiles(activeKeys);

    // 5. 정렬된 순서대로 비동기 요청 실행
    const queue = getScheduledQueue(tx, ty);

    if (queue.length > 0) {
      setStatus('loading');

      const tasks = queue.map((req) => loadTile(req.tx, req.ty, req.key));

      // 7. 상태 관리
      Promise.all(tasks).finally(() => {
        // [race-condition-check] 배치가 완료되었을 때 여전히 최신 배치인지 확인
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
