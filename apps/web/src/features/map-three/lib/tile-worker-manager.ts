import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';
import type {
  TileWorkerInput,
  TileWorkerResponse,
} from '@pixelet/shared/entities/osm/model/worker-types';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { delay } from '@pixelet/shared/shared/lib/aync-utils';

const { API_MODE } = OSM_CONFIG;

/**
 * [TaskCallbacks]
 * 특정 타일 요청에 대한 상태와 제어권을 관리합니다.
 */
interface TaskCallbacks {
  /** [Deduplication] 중복 요청 시 기존 Promise를 반환하여 리소스 낭비를 방지합니다. */
  promise: Promise<GeometryTile>;
  resolve: (data: GeometryTile) => void;
  reject: (error: any) => void;
  /** [Progressive Update] 데이터 가공 단계별(지형 -> 건물 등) 콜백 */
  onProgress?: (data: Partial<GeometryTile>) => void;
}

/**
 * [TileWorkerManager]
 * Web Worker를 관리하고 메인 스레드와의 통신을 중재하는 매니저입니다.
 * 모든 요청은 requestId(tx:ty)를 통해 추적 및 중복 제거(Deduplication)됩니다.
 */
class TileWorkerManager {
  private worker: Worker | null = null;
  /** 진행 중인 작업 목록 (Key: 'tx:ty') */
  private callbacks = new Map<string, TaskCallbacks>();

  constructor() {
    this.initWorker();
  }

  /**
   * [Internal] 워커 초기화 및 메시지 핸들러 등록
   */
  private initWorker() {
    // Vite 환경의 Worker 생성 표준 방식
    this.worker = new Worker(
      new URL('../../../workers/osm.worker.ts', import.meta.url),
      { type: 'module' },
    );

    this.worker.onmessage = (e: MessageEvent<TileWorkerResponse>) => {
      const { type, requestId, data, error } = e.data;
      const callback = this.callbacks.get(requestId);

      if (!callback) return;

      switch (type) {
        case 'progress':
          // 점진적 업데이트: 가공 중인 부분 데이터를 UI에 즉시 반영
          if (data) callback.onProgress?.(data);
          break;

        case 'complete':
          // 최종 완료: 데이터 전달 후 콜백 맵에서 삭제 (메모리 관리)
          if (data) {
            callback.resolve(data as GeometryTile);
            this.callbacks.delete(requestId);
          }
          break;

        case 'error':
          // 에러 발생: 상위 호출자에게 에러 전파 후 정리
          console.error(`[WorkerManager] Error in tile ${requestId}:`, error);
          callback.reject(error);
          this.callbacks.delete(requestId);
          break;
      }
    };

    /**
     * [Global Error Handler]
     * 워커 내부의 예기치 못한 크래시 발생 시 대기 중인 모든 요청을 Reject 처리합니다.
     */
    this.worker.onerror = (err) => {
      console.error('[WorkerManager] Worker global error:', err);
      this.callbacks.forEach((cb) => cb.reject(err));
      this.callbacks.clear();
    };
  }

  /**
   * [requestTile]
   * 타일 생성을 요청합니다. 이미 요청된 타일은 기존 Promise를 반환합니다.
   * * @param tx - 타일 X 인덱스
   * @param ty - 타일 Y 인덱스
   * @param onProgress - 부분 데이터 수신 시 콜백
   */
  public async requestTile(
    tx: number,
    ty: number,
    onProgress?: (data: Partial<GeometryTile>) => void,
  ): Promise<GeometryTile> {
    const requestId = `${tx}:${ty}`;

    // 1. 중복 요청 확인 (Deduplication)
    if (this.callbacks.has(requestId)) {
      return this.callbacks.get(requestId)!.promise;
    }

    // [중요] 동시 요청 시 서버 부하 분산을 위해 약 50~100ms의 시차를 줍니다.
    // 이는 9개 타일 요청 시 마지막 타일까지 약 0.5초의 간격을 만들어 서버 폭주를 막습니다.
    await delay(Math.random() * 100);

    // 2. Promise 제어권 추출
    let resolveFn: (data: GeometryTile) => void;
    let rejectFn: (error: any) => void;

    const promise = new Promise<GeometryTile>((resolve, reject) => {
      resolveFn = resolve;
      rejectFn = reject;
    });

    // 3. 콜백 등록
    this.callbacks.set(requestId, {
      promise,
      resolve: resolveFn!,
      reject: rejectFn!,
      onProgress,
    });

    // 4. [주입] 워커에게 보낼 구조화된 데이터 생성
    // 메인 스레드의 최신 설정을 스냅샷으로 전달하여 워커의 순수성을 유지합니다.
    const input: TileWorkerInput = {
      coord: { tx, ty },
      options: {
        apiMode: API_MODE,
      },
      meta: {
        requestId,
      },
    };

    this.worker?.postMessage(input);

    return promise;
  }

  /**
   * 워커를 강제 종료하고 상태를 초기화합니다.
   */
  public terminate() {
    this.worker?.terminate();
    this.worker = null;
    this.callbacks.clear();
  }
}

export const tileWorkerManager = new TileWorkerManager();
