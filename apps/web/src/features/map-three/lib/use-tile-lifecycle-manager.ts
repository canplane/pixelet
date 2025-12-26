import { useEffect, useRef } from 'react';
import { useMapStore } from '@/features/map-three/model/map-store';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
// [변경 1] 직접 처리 함수 대신 워커 매니저를 가져옵니다.
import { tileWorkerManager } from '@/features/map-three/lib/tile-worker-manager';

interface TileRequest {
  tx: number;
  tz: number;
  key: string;
  distance: number;
}

/**
 * [use-tile-lifecycle-manager]
 * Web Worker를 통해 백그라운드에서 타일을 로드하고,
 * 점진적 렌더링(Progressive Rendering) 결과를 UI에 반영합니다.
 *
 * [Scheduling]
 * 중심 좌표를 기준으로 가까운 타일(Radius Sort)부터 우선적으로 Worker에 요청하여,
 * 사용자의 시야 중심부(Ripple Effect)가 가장 먼저 로딩되도록 스케줄링합니다.
 */
export function useTileLifecycleManager(
  center: { x: number; z: number } | null,
) {
  const setWorldTile = useMapStore((state) => state.setWorldTile);
  const removeWorldTile = useMapStore((state) => state.removeWorldTile);
  const setStatus = useMapStore((state) => state.setStatus);

  const lastCenterTileKey = useRef<string>('');
  const processingTiles = useRef<Set<string>>(new Set());

  useEffect(() => {
    const { TILE_SIZE, DEFAULT_RENDER_RADIUS } = OSM_CONFIG;

    if (!center) return;

    // 1. 현재 중심 타일 계산
    const tx = Math.floor(center.x / TILE_SIZE);
    const tz = Math.floor(center.z / TILE_SIZE);
    const currentKey = `${tx}:${tz}`;

    if (lastCenterTileKey.current === currentKey) return;
    lastCenterTileKey.current = currentKey;

    const activeKeys = new Set<string>();
    const pendingRequests: TileRequest[] = [];

    // 2. 가시 범위 순회 및 후보군 수집
    for (let dx = -DEFAULT_RENDER_RADIUS; dx <= DEFAULT_RENDER_RADIUS; dx++) {
      for (let dz = -DEFAULT_RENDER_RADIUS; dz <= DEFAULT_RENDER_RADIUS; dz++) {
        const targetTx = tx + dx;
        const targetTz = tz + dz;
        const key = `${targetTx}:${targetTz}`;
        activeKeys.add(key);

        const { worldTiles } = useMapStore.getState();

        // 3. 로드 요청 대기열 추가 (이미 로드된 타일 제외)
        if (!worldTiles.has(key) && !processingTiles.current.has(key)) {
          // [Priority Scheduling] 중심부와의 거리(가중치) 계산
          // 유클리드 거리 제곱 (sqrt 연산 최적화)
          const distance = dx * dx + dz * dz;
          pendingRequests.push({ tx: targetTx, tz: targetTz, key, distance });
        }
      }
    }

    // 4. [핵심] 거리순 정렬 (가까운 타일 -> 먼 타일)
    // Worker는 FIFO(선입선출)로 메시지를 처리하므로, 요청 순서가 곧 로딩 순서가 됩니다.
    pendingRequests.sort((a, b) => a.distance - b.distance);

    // 5. 정렬된 순서대로 비동기 요청 실행
    const tasks = pendingRequests.map((req) => {
      const { tx: targetTx, tz: targetTz, key } = req;

      processingTiles.current.add(key);

      // [변경 2] tileWorkerManager를 통해 요청합니다.
      return tileWorkerManager
        .requestTile(
          targetTx,
          targetTz,
          // [점진적 업데이트] Worker가 중간 결과를 보낼 때마다 실행됨
          (partialTile) => {
            setWorldTile(key, partialTile);
          },
        )
        .then((finalTile) => {
          // [최종 완료]
          if (finalTile && finalTile.grids) {
            setWorldTile(key, finalTile);
          }
        })
        .catch((err) => {
          console.error(`[Lifecycle] Worker error ${key}:`, err);
        })
        .finally(() => {
          processingTiles.current.delete(key);
        });
    });

    // 6. 범위 밖 타일 정리 (메모리 관리)
    const { worldTiles } = useMapStore.getState();
    worldTiles.forEach((_, key) => {
      // 현재 보고 있는 범위(activeKeys)에도 없고, 로딩 중도 아니라면 삭제
      if (!activeKeys.has(key) && !processingTiles.current.has(key)) {
        removeWorldTile(key);
      }
    });

    // 7. 상태 관리
    if (tasks.length > 0) {
      setStatus('loading');
      Promise.all(tasks)
        .then(() => setStatus('success'))
        .catch(() => setStatus('success')); // 부분 에러는 허용
    } else {
      const { worldTiles: currentTiles } = useMapStore.getState();
      if (currentTiles.size > 0) setStatus('success');
    }
  }, [center, setWorldTile, removeWorldTile, setStatus]); // project/unproject 의존성 제거됨
}
