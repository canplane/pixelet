/// <reference lib="webworker" />

// [중요] 기존 로직 함수 import
import { fetchAndProcessTile } from '@/entities/osm/model/tile-processor';

// [중요] 좌표 변환 함수 import
import { projection } from '@/shared/lib/geo/projection';
const { project, unproject } = projection;

// ✨ [수정 1] self를 Worker 스코프로 타입 단언 (TS 에러 해결 핵심)
const worker = self as unknown as DedicatedWorkerGlobalScope;

worker.onmessage = async (e: MessageEvent) => {
  const { tx, tz, id } = e.data;

  try {
    // 타일 처리 실행
    const result = await fetchAndProcessTile(
      tx,
      tz,
      project,
      unproject,
      (partialTile) => {
        // [점진적 업데이트]
        // self 대신 위에서 정의한 worker를 사용
        worker.postMessage({
          type: 'progress',
          id,
          data: partialTile,
        });
      },
    );

    // [성능 최적화] Transferable Object
    const transferables: Transferable[] = [];
    if (result.grids) {
      if (result.grids.base.buffer)
        transferables.push(result.grids.base.buffer);
      if (result.grids.water.buffer)
        transferables.push(result.grids.water.buffer);
      if (result.grids.transport.buffer)
        transferables.push(result.grids.transport.buffer);
      if (result.grids.structure.buffer)
        transferables.push(result.grids.structure.buffer);
    }

    // ✨ [수정 2] self 대신 worker 사용 (이제 에러가 사라집니다)
    // Worker의 postMessage는 두 번째 인자로 Transferable[]을 허용합니다.
    worker.postMessage({ type: 'complete', id, data: result }, transferables);
  } catch (error) {
    worker.postMessage({
      type: 'error',
      id,
      error: error instanceof Error ? error.message : 'Unknown Worker Error',
    });
  }
};
