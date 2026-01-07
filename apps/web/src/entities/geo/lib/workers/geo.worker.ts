/// <reference lib="webworker" />

import { fetchTile } from '@/entities/geo/lib/fetch/tile-fetcher';
import type { TileWorkerInput } from '@/entities/geo/model/worker-protocol';

// 🎯 [Standard Integration]: 직렬화를 위해 코덱을 가져옵니다.
import { SparseQuadtreeCodec } from '@pixelet/core/shared/lib/structs/sparse-quadtree-codec';
import { TILE_SURFACE_KEYS } from '@pixelet/core/entities/geo/model/tiles';

// self를 Worker 스코프로 타입 단언
const worker = self as unknown as DedicatedWorkerGlobalScope;

worker.onmessage = async (e: MessageEvent<TileWorkerInput>) => {
  const { coord, options, meta } = e.data;
  const { requestId } = meta;

  try {
    // 타일 처리 실행
    const result = await fetchTile(coord, options, (partialTile) => {
      // [점진적 업데이트]
      // 여기서는 Transferable을 쓰지 않습니다. (부분 데이터는 복사 비용이 작음)
      worker.postMessage({
        type: 'progress',
        requestId,
        data: partialTile,
      });
    });

    /**
     * 🎯 [The Standard: Data Serialization]
     * SparseQuadtree 인스턴스는 내부에 '생성자 함수'를 포함하고 있어 postMessage로 직접 보낼 수 없습니다.
     * 따라서 코덱을 이용해 ArrayBuffer로 인코딩하여 데이터만 전송합니다.
     */
    const { surface, ele } = result.base;
    const bufferSet = new Set<ArrayBuffer>();

    // 1. Elevation (고도) 직렬화
    let encodedEle = null;
    if (ele) {
      encodedEle = SparseQuadtreeCodec.encode(ele);
      bufferSet.add(encodedEle);
    }

    // 2. Surface (지형 채널) 직렬화
    const encodedSurface: any = {};
    if (surface) {
      TILE_SURFACE_KEYS.forEach((key) => {
        const tree = surface[key];
        if (tree) {
          const buf = SparseQuadtreeCodec.encode(tree);
          encodedSurface[key] = buf;
          bufferSet.add(buf);
        }
      });
    }

    // 🚀 [Zero-Copy] 이전 가능한 객체(Transferable) 목록 생성
    const transferables = Array.from(bufferSet);

    // 메인 스레드로 최종 결과 전송
    worker.postMessage(
      {
        type: 'complete',
        requestId,
        data: {
          ...result,
          base: {
            ...result.base,
            ele: encodedEle,
            surface: encodedSurface,
          },
        },
      },
      transferables,
    );
  } catch (error) {
    console.error(`[WorkerManager] Error in tile ${requestId}:`, error);
    worker.postMessage({
      type: 'error',
      requestId,
      error: error instanceof Error ? error.message : 'Unknown error',
    });
  }
};
