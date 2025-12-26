/// <reference lib="webworker" />

// [중요] 기존 로직 함수 import
import { fetchAndProcessTile } from '@pixelet/shared/features/osm/lib/tile-processor';

// ✨ [Type Definition] 워커가 받는 메시지 타입 정의 (안전성 강화)
interface WorkerInput {
  tx: number;
  tz: number;
  id: string;
}

// ✨ [수정 1] self를 Worker 스코프로 타입 단언
const worker = self as unknown as DedicatedWorkerGlobalScope;

worker.onmessage = async (e: MessageEvent<WorkerInput>) => {
  const { tx, tz, id } = e.data;

  try {
    // 타일 처리 실행
    const result = await fetchAndProcessTile(tx, tz, (partialTile) => {
      // [점진적 업데이트]
      // 여기서는 Transferable을 쓰지 않습니다. (부분 데이터는 복사 비용이 작음)
      worker.postMessage({
        type: 'progress',
        id,
        data: partialTile,
      });
    });

    // [성능 최적화] Transferable Object 준비
    // ArrayBuffer의 소유권을 메인 스레드로 '이동'시켜 복사 비용을 0으로 만듭니다.
    const transferables: Transferable[] = [];

    if (result.grids) {
      // 각 채널(Uint8Array)의 underlying buffer를 추출
      if (result.grids.base?.buffer)
        transferables.push(result.grids.base.buffer);
      if (result.grids.water?.buffer)
        transferables.push(result.grids.water.buffer);
      if (result.grids.transport?.buffer)
        transferables.push(result.grids.transport.buffer);
      if (result.grids.structure?.buffer)
        transferables.push(result.grids.structure.buffer);
    }

    // ✨ [Complete] 결과 전송 (Zero-Copy)
    // worker.postMessage의 두 번째 인자로 버퍼 리스트를 넘기면 됩니다.
    worker.postMessage({ type: 'complete', id, data: result }, transferables);
  } catch (error) {
    worker.postMessage({
      type: 'error',
      id,
      error: error instanceof Error ? error.message : 'Unknown Worker Error',
    });
  }
};
