import { useEffect, useRef } from 'react';
import { useMapStore } from '@/features/map-three/model/map-store';
import { OSM_CONFIG } from '../config/osm-config';
import { fetchAndProcessTile } from './tile-processor';
import type { Location } from '@/shared/types/map';

/**
 * [use-tile-lifecycle-manager]
 * 카메라 중심점(Center)을 기준으로 가시 범위 내 타일을 로드하고 리소스를 해제합니다.
 * 타일 데이터는 절대 미터 좌표계를 기준으로 관리됩니다.
 */
export function useTileLifecycleManager(
  center: { x: number; z: number } | null,
  unproject: (pos: { x: number; y: number; z: number }) => Location,
) {
  const project = useMapStore((state) => state.project);
  const setWorldTile = useMapStore((state) => state.setWorldTile);
  const removeWorldTile = useMapStore((state) => state.removeWorldTile);
  const setStatus = useMapStore((state) => state.setStatus);

  const lastCenterTileKey = useRef<string>('');
  const processingTiles = useRef<Set<string>>(new Set());

  useEffect(() => {
    const { TILE_SIZE, DEFAULT_RENDER_RADIUS } = OSM_CONFIG;

    if (!center || !project) return;

    // 현재 좌표가 속한 타일 인덱스 계산
    const tx = Math.floor(center.x / TILE_SIZE);
    const tz = Math.floor(center.z / TILE_SIZE);
    const currentKey = `${tx}:${tz}`;

    // 타일 경계를 넘지 않는 미세 이동 시 연산 스킵
    if (lastCenterTileKey.current === currentKey) return;
    lastCenterTileKey.current = currentKey;

    const activeKeys = new Set<string>();
    const tasks: Promise<void>[] = [];

    // 가시 범위 내 타일 인덱스 순회
    for (let dx = -DEFAULT_RENDER_RADIUS; dx <= DEFAULT_RENDER_RADIUS; dx++) {
      for (let dz = -DEFAULT_RENDER_RADIUS; dz <= DEFAULT_RENDER_RADIUS; dz++) {
        const targetTx = tx + dx;
        const targetTz = tz + dz;
        const key = `${targetTx}:${targetTz}`;
        activeKeys.add(key);

        const { worldTiles } = useMapStore.getState();

        // 중복 로드 방지 가드
        if (!worldTiles.has(key) && !processingTiles.current.has(key)) {
          processingTiles.current.add(key);

          tasks.push(
            fetchAndProcessTile(targetTx, targetTz, project, unproject)
              .then((data) => {
                // [보강] 데이터가 grid를 포함한 완전한 WorldTile 형상인지 확인 후 저장
                if (data && data.grid) {
                  setWorldTile(key, data);
                }
              })
              .catch((err) => {
                console.error(`[Lifecycle] Tile error ${key}:`, err);
              })
              .finally(() => {
                processingTiles.current.delete(key);
              }),
          );
        }
      }
    }

    // 범위 밖 타일 메모리 해제
    const { worldTiles } = useMapStore.getState();
    worldTiles.forEach((_, key) => {
      // 현재 활성 키가 아니고, 처리 중인 타일도 아닌 경우에만 제거
      if (!activeKeys.has(key) && !processingTiles.current.has(key)) {
        removeWorldTile(key);
      }
    });

    // 전체 작업 상태 동기화
    if (tasks.length > 0) {
      setStatus('loading');
      Promise.all(tasks)
        .then(() => {
          setStatus('success');
        })
        .catch(() => {
          setStatus('error');
        });
    } else {
      // 로드할 타일이 없더라도 타일이 이미 존재한다면 success 유지
      const { worldTiles: currentTiles } = useMapStore.getState();
      if (currentTiles.size > 0) setStatus('success');
    }
  }, [center, project, unproject, setWorldTile, removeWorldTile, setStatus]);
}
