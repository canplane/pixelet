/// <reference lib="webworker" />

import { fetchTile } from '@/entities/geo/lib/fetch/tile-fetcher';
import type { TileWorkerInput } from '@/entities/geo/model/worker-protocol';

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

    // 🎯 [Transferable Optimization]
    const bufferSet = new Set<ArrayBuffer>();

    /**
     * [Helper] Transfer 가능한 순수 ArrayBuffer만 선별합니다.
     * SharedArrayBuffer는 공유 메모리이므로 전송 리스트에 넣으면 에러가 발생합니다.
     */
    const collectTransferable = (view: Uint8Array | Uint16Array) => {
      // 뷰가 바라보는 실제 메모리 덩어리
      const buf = view.buffer;
      // SharedArrayBuffer 제외하고, 순수 ArrayBuffer만 수집
      if (buf instanceof ArrayBuffer) {
        bufferSet.add(buf);
      }
    };

    // 1. Channels (8-bit) 수집
    if (result.terrain.channels) {
      Object.values(result.terrain.channels).forEach(collectTransferable);
    }

    // 2. Elevation (16-bit) 수집
    if (result.terrain.elevation) {
      collectTransferable(result.terrain.elevation);
    }

    // Set을 다시 배열로 변환하여 전송
    const transferables = Array.from(bufferSet);

    /**
     * [Zero-Copy Transfer]
     * transferables 리스트에 담긴 버퍼만 소유권이 이전됩니다.
     */
    worker.postMessage(
      { type: 'complete', requestId, data: result },
      transferables,
    );
  } catch (error) {
    worker.postMessage({
      type: 'error',
      requestId,
      error: error instanceof Error ? error.message : 'Unknown Worker Error',
    });
  }
};
