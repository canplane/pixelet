import type { WorldTile } from '@pixelet/shared/entities/osm/model/tiles';

/**
 * [WorkerResponse] 워커로부터 수신하는 메시지 타입 정의
 */
type WorkerResponse =
  | { type: 'progress'; id: string; data: Partial<WorldTile> }
  | { type: 'complete'; id: string; data: WorldTile }
  | { type: 'error'; id: string; error: any };

/**
 * [TaskCallbacks] 특정 타일 요청에 대한 콜백 핸들러 저장소
 */
interface TaskCallbacks {
  /**
   * [Deduplication]
   * 중복 요청 발생 시, 새로 워커를 호출하지 않고 기존 작업을 반환하기 위해
   * 진행 중인 Promise 객체 자체를 저장합니다.
   */
  promise: Promise<WorldTile>;
  resolve: (data: WorldTile) => void;
  reject: (error: any) => void;
  onProgress?: (data: Partial<WorldTile>) => void;
}

class TileWorkerManager {
  private worker: Worker | null = null;
  /**
   * 진행 중인 작업 목록 (Key: 'tx:tz')
   * 중복 요청을 방지하고 완료 시 Promise를 해결하기 위해 관리합니다.
   */
  private callbacks = new Map<string, TaskCallbacks>();

  constructor() {
    this.initWorker();
  }

  private initWorker() {
    // Vite에서 Worker를 import하는 표준 방식
    // (경로가 실제 파일 위치와 맞는지 꼭 확인하세요!)
    this.worker = new Worker(
      new URL('../../../workers/osm.worker.ts', import.meta.url),
      {
        type: 'module',
      },
    );

    this.worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
      // [수정] data, error를 미리 꺼내지 않고 response 객체 자체를 참조합니다.
      const response = e.data;
      const { type, id } = response;

      const callback = this.callbacks.get(id);

      // 이미 처리되었거나 취소된 작업이라면 무시
      if (!callback) return;

      switch (type) {
        case 'progress':
          // [Progressive Update] 부분 업데이트 (지형 -> 건물 -> ...)
          // TS가 'progress' 타입임을 알기 때문에 .data 접근이 가능해집니다.
          if (response.type === 'progress' && response.data) {
            // 중복 요청이 들어와도 최초 요청의 onProgress 하나만 실행되므로 효율적입니다.
            callback.onProgress?.(response.data);
          }
          break;

        case 'complete':
          // [Done] 최종 완료
          if (response.type === 'complete') {
            callback.resolve(response.data);
            this.callbacks.delete(id); // 메모리 누수 방지를 위해 즉시 삭제
          }
          break;

        case 'error':
          // [Fail] 에러 처리
          if (response.type === 'error') {
            console.error(
              `[WorkerManager] Error in tile ${id}:`,
              response.error,
            );
            callback.reject(response.error);
            this.callbacks.delete(id);
          }
          break;
      }
    };

    // [Global Error Handler]
    // 워커 스크립트 문법 오류 등으로 워커가 죽었을 때,
    // 대기 중인 모든 Promise를 Reject 처리하여 UI가 영원히 로딩 상태로 남는 것을 방지합니다.
    this.worker.onerror = (err) => {
      console.error('[WorkerManager] Worker global error:', err);
      this.callbacks.forEach((cb) => cb.reject(err));
      this.callbacks.clear();
    };
  }

  /**
   * [requestTile] 타일 생성 요청 (Promise 반환)
   * - 중복 요청 시(이미 로딩 중), 새로 요청하지 않고 기존 Promise를 반환합니다.
   * - 이를 통해 빠른 이동 시 요청이 덮어씌워져(Overwrite) 타일이 누락되는 현상을 방지합니다.
   */
  public requestTile(
    tx: number,
    tz: number,
    onProgress?: (data: Partial<WorldTile>) => void,
  ): Promise<WorldTile> {
    const id = `${tx}:${tz}`;

    // 워커가 혹시 죽었거나 없으면 재실행
    if (!this.worker) this.initWorker();

    // ✨ [Deduplication] 이미 진행 중인 요청이 있다면 기존 Promise 재사용
    // 기존에는 여기서 덮어쓰기가 발생하여 이전 요청의 resolve가 소실되는 버그가 있었습니다.
    if (this.callbacks.has(id)) {
      // console.log(`[WorkerManager] Deduplicated request for ${id}`);
      return this.callbacks.get(id)!.promise;
    }

    // Promise의 제어권(resolve, reject)을 추출하여 Map에 저장
    let resolveFn: (data: WorldTile) => void;
    let rejectFn: (error: any) => void;

    const promise = new Promise<WorldTile>((resolve, reject) => {
      resolveFn = resolve;
      rejectFn = reject;
    });

    // 콜백 맵에 등록 (Promise 객체 포함)
    this.callbacks.set(id, {
      promise,
      resolve: resolveFn!, // Executor는 동기 실행되므로 할당 보장됨
      reject: rejectFn!,
      onProgress,
    });

    // 워커에게 작업 지시
    this.worker?.postMessage({ tx, tz, id });

    return promise;
  }

  public terminate() {
    this.worker?.terminate();
    this.worker = null;
    this.callbacks.clear();
  }
}

// 싱글톤 인스턴스로 내보내기
export const tileWorkerManager = new TileWorkerManager();
