import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';

// 2. 기초 타입 (Coord 등)
import type { Tile } from '@pixelet/core/entities/osm/model/tiles';
import { createEmptyTile } from '@pixelet/core/entities/osm/lib/utils/tile-utils';

export interface TileState {
  // --- [Tile Data Management] ---
  /** 현재 메모리에 로드된 타일 데이터 저장소 (Key: "tx:ty") */
  tilesMap: Map<string, Tile>;
  /**
   * [set-tile] (Partial Update Supported)
   * 타일 데이터를 새로 추가하거나, 이미 존재하는 타일에 데이터를 병합(Merge)합니다.
   * 예: Worker로부터 지형 데이터 수신 후, 나중에 POI 데이터 수신 시 병합
   */
  setTile: (key: string, tile: Partial<Tile>) => void;
  /** 특정 타일을 메모리에서 즉시 해제합니다. */
  removeTile: (key: string) => void;
  /** 가시 범위 밖의 불필요한 타일들을 일괄 정리(GC)합니다. */
  clearOldTiles: (neededKeys: string[]) => string[];
}

export const useTileStore = create<TileState>()(
  immer((set, get) => ({
    tilesMap: new Map<string, Tile>(),

    /** [setTile] Immer의 진가가 발휘되는 곳 */
    setTile: (key, partialTile) =>
      set((state) => {
        const existing = state.tilesMap.get(key);

        if (existing) {
          // 1. 기존 타일이 있다면 Immer Proxy를 통해 해당 부분만 핀포인트 업데이트
          Object.assign(existing, partialTile);
        } else if (partialTile.coord) {
          // 2. 신규 타일인 경우
          // createEmptyTile(coord) 내부에서 key: getTileKey(coord)가 수행됨을 전제합니다.
          const newTile = {
            ...createEmptyTile(partialTile.coord),
            ...partialTile,
          };
          state.tilesMap.set(key, newTile);
        }
      }),

    removeTile: (key) =>
      set((state) => {
        state.tilesMap.delete(key); // 🎯 수동 복사 없이 직접 명령
      }),

    /**
     * [clearOldTiles]
     * 성능 최적화(Set)와 메모리 해제(Return removed)를 동시에 처리합니다.
     */
    clearOldTiles: (neededKeys: string[]) => {
      // 1. 성능 최적화: 인자로 받은 리스트를 Set으로 변환 (조회 O(1))
      const neededSet = new Set(neededKeys);
      const removedKeys: string[] = [];

      // 2. 삭제 로직 수행
      set((state) => {
        // Map의 keys를 배열로 스냅샷 찍어 안전하게 순회
        const currentKeys = Array.from(state.tilesMap.keys()) as string[];
        currentKeys.forEach((key) => {
          if (!neededSet.has(key)) {
            state.tilesMap.delete(key); // 스토어에서 제거
            removedKeys.push(key); // 제거된 목록 기록
          }
        });
      });

      // 3. 삭제된 목록 반환 (메인 스레드에서 Three.js 오브젝트 dispose 등에 활용)
      return removedKeys;
    },
  })),
);
