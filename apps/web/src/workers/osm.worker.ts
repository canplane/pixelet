/// <reference lib="webworker" />

import { fetchAndProcessTile } from '@pixelet/shared/features/osm/lib/tile-processor';
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

    /**
     * [성능 최적화: Zero-Copy]
     * ArrayBuffer의 소유권을 메인 스레드로 '이동'시켜 복사 비용을 없앱니다.
     * 🎯 [개선] 하드코딩 대신 Object.values를 통해 모든 그리드 채널의 버퍼를 자동 추출합니다.
     */
    const transferables: ArrayBuffer[] = Object.values(result.grids)
      .map((grid) => grid.buffer)
      .filter((buffer) => buffer instanceof ArrayBuffer);

    // ✨ 최종 결과 전송
    worker.postMessage(
      { type: 'complete', requestId, data: result },
      transferables,
    );

    // ✨ [Complete] 결과 전송 (Zero-Copy)
    // worker.postMessage의 두 번째 인자로 버퍼 리스트를 넘기면 됩니다.
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
