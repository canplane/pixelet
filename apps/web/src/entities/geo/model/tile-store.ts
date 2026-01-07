/**
 * @file tile-store.ts
 * 타일 데이터(Tile)를 통합 관리하는 중앙 스토어입니다.
 * [Architecture]
 * 1. tileMap: 원본 데이터(Tile)를 보관하며 워커로부터 들어오는 SparseQuadtree 기반 업데이트를 수용합니다.
 * 2. Optimized Merge: 단순 할당이 아닌, 계층형 데이터 구조(Base > Surface)를 고려한 깊은 병합을 수행합니다.
 * 3. Tiered Cleanup: [Soft Unload]와 [Hard Delete]를 분리하여 VRAM 효율을 극대화합니다.
 */

import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import type { Tile } from '@pixelet/core/entities/geo/model/tiles';
import { createEmptyTile } from '@pixelet/core/entities/geo/lib/utils/tile-factory';

export interface TileState {
  // --- [Tile Data Management] ---
  /** 📦 Tile 모델 저장소 (순수 데이터) */
  tileMap: Map<string, Tile>;

  // --- [Selectors] ---
  /** 특정 좌표의 순수 데이터(Tile)를 가져옵니다. */
  getTile: (key: string) => Tile | undefined;

  // --- [Actions] ---
  /**
   * [setTile] (Partial Update Supported)
   * 타일 데이터를 새로 추가하거나, 이미 존재하는 타일에 데이터를 병합(Merge)합니다.
   * 💡 SparseQuadtree 인스턴스는 참조가 중요하므로 계층별로 정밀하게 병합합니다.
   */
  setTile: (key: string, partialTile: Partial<Tile>) => void;

  /** 특정 타일을 메모리에서 즉시 해제합니다. [Hard Delete] */
  clearTile: (key: string) => void;

  /** * [unloadBuildLayer]
   * 🎯 [h * h * (h/2) Optimization]
   * 타일의 기본 지형(Base)은 유지하되, VRAM 점유가 큰 빌드(Chunk) 데이터만 제거합니다. [Soft Unload]
   */
  unloadBuildLayer: (key: string) => void;
}

export const useTileStore = create<TileState>()(
  immer((set, get) => ({
    tileMap: new Map(),

    getTile: (key) => get().tileMap.get(key),

    setTile: (key, partialTile) =>
      set((state) => {
        let tile = state.tileMap.get(key);

        if (!tile) {
          if (!partialTile.coord) return;
          tile = createEmptyTile(partialTile.coord);
          state.tileMap.set(key, tile);
        }

        // 1. [Structural Data Merge]
        // SparseQuadtree 인스턴스는 '교체' 방식으로 병합하여 무결성 유지
        if (partialTile.base) {
          // A. Elevation 병합 (SparseQuadtree <Int16Array>)
          if (partialTile.base.ele) tile.base.ele = partialTile.base.ele;

          // B. Surface 채널별 병합 (SparseQuadtree <Uint8Array> 들의 집합)
          if (partialTile.base.surface) {
            for (const sKey of Object.keys(partialTile.base.surface) as Array<keyof typeof partialTile.base.surface>) {
              const subTree = partialTile.base.surface[sKey];
              if (subTree) tile.base.surface[sKey] = subTree;
            }
          }
        }

        // 2. [Build System]
        // 🎯 Soft Unload(undefined) 상태에서도 안전하게 병합될 수 있도록 가드 처리
        if (partialTile.build) {
          tile.build = { ...(tile.build || {}), ...partialTile.build };
        }

        if (partialTile.pois) tile.pois = partialTile.pois;

        // 3. [Metadata & Collections Merge]
        // 🎯 서버에서 온 metadata(version, timestamp 등)를 신뢰하고 전파합니다.
        if (partialTile.metadata) {
          tile.metadata = { ...tile.metadata, ...partialTile.metadata };
        }
      }),

    clearTile: (key) =>
      set((state) => {
        state.tileMap.delete(key);
      }),

    unloadBuildLayer: (key) =>
      set((state) => {
        const tile = state.tileMap.get(key);
        // 🎯 VRAM 해제를 위해 build 데이터 초기화 (GC 대상 유도)
        if (tile) {
          tile.build = undefined;
        }
      }),
  })),
);
