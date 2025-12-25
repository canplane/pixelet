import { useEffect, useRef } from 'react';
import { useMapStore } from '@/features/map-three/model/map-store';
import { OSM_CONFIG } from '../config/osm-config';
// [변경 1] 직접 처리 함수 대신 워커 매니저를 가져옵니다.
import { tileWorkerManager } from '@/features/map-three/lib/tile-worker-manager';

/**
 * [use-tile-lifecycle-manager]
 * Web Worker를 통해 백그라운드에서 타일을 로드하고,
 * 점진적 렌더링(Progressive Rendering) 결과를 UI에 반영합니다.
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
    const tasks: Promise<void>[] = [];

    // 2. 가시 범위 순회
    for (let dx = -DEFAULT_RENDER_RADIUS; dx <= DEFAULT_RENDER_RADIUS; dx++) {
      for (let dz = -DEFAULT_RENDER_RADIUS; dz <= DEFAULT_RENDER_RADIUS; dz++) {
        const targetTx = tx + dx;
        const targetTz = tz + dz;
        const key = `${targetTx}:${targetTz}`;
        activeKeys.add(key);

        const { worldTiles } = useMapStore.getState();

        // 3. 로드 요청 (Worker 사용)
        if (!worldTiles.has(key) && !processingTiles.current.has(key)) {
          processingTiles.current.add(key);

          // [변경 2] tileWorkerManager를 통해 요청합니다.
          // 더 이상 project/unproject 함수를 인자로 넘기지 않습니다. (Worker가 알아서 함)
          const task = tileWorkerManager
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

          tasks.push(task);
        }
      }
    }

    // 4. 범위 밖 타일 정리
    const { worldTiles } = useMapStore.getState();
    worldTiles.forEach((_, key) => {
      if (!activeKeys.has(key) && !processingTiles.current.has(key)) {
        removeWorldTile(key);
      }
    });

    // 5. 상태 관리
    if (tasks.length > 0) {
      setStatus('loading');
      Promise.all(tasks)
        .then(() => setStatus('success'))
        .catch(() => setStatus('success')); // 부분 에러는 허용
    } else {
      const { worldTiles: currentTiles } = useMapStore.getState();
      if (currentTiles.size > 0) setStatus('success');
    }
  }, [center, setWorldTile, removeWorldTile, setStatus]); // project/unproject 의존성 제거 가능
}
