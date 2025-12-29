/// <reference lib="webworker" />

import { fetchAndProcessTile } from '@pixelet/shared/features/osm/lib/tile-processor.client';
import type { TileWorkerInput } from '@pixelet/shared/entities/osm/model/worker-types';

// ✨ [수정 1] self를 Worker 스코프로 타입 단언
const worker = self as unknown as DedicatedWorkerGlobalScope;

worker.onmessage = async (e: MessageEvent<TileWorkerInput>) => {
  const { coord, options, meta } = e.data;
  const { requestId } = meta;

  try {
    // 타일 처리 실행
    const result = await fetchAndProcessTile(coord, options, (partialTile) => {
      // [점진적 업데이트]
      // 여기서는 Transferable을 쓰지 않습니다. (부분 데이터는 복사 비용이 작음)
      worker.postMessage({
        type: 'progress',
        requestId,
        data: partialTile,
      });
    });

    // 🎯 [Optimization]: 중복 버퍼 제거 로직 추가
    const bufferSet = new Set<ArrayBuffer>();

    /**
     * [성능 최적화: Zero-Copy]
     * ArrayBuffer의 소유권을 메인 스레드로 '이동'시켜 복사 비용을 없앱니다.
     * 🎯 [개선] 하드코딩 대신 Object.values를 통해 모든 그리드 채널의 버퍼를 자동 추출합니다.
     */
    Object.values(result.grids).forEach((grid) => {
      if (grid instanceof Uint8Array && grid.buffer instanceof ArrayBuffer) {
        bufferSet.add(grid.buffer);
      }
    });

    const transferables = Array.from(bufferSet);

    /**
     * [Zero-Copy Transfer]
     * 버퍼의 소유권을 메인 스레드로 완전히 넘깁니다.
     * 이제 워커에서는 이 버퍼를 참조할 수 없게 되지만, 전송 속도는 최상입니다.
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
