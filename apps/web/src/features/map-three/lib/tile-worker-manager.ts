import type { WorldTile } from '@/features/map-three/model/map-store';

// 워커와 주고받는 메시지 타입 정의
type WorkerResponse =
  | { type: 'progress'; id: string; data: Partial<WorldTile> }
  | { type: 'complete'; id: string; data: WorldTile }
  | { type: 'error'; id: string; error: any };

interface TaskCallbacks {
  resolve: (data: WorldTile) => void;
  reject: (error: any) => void;
  onProgress?: (data: Partial<WorldTile>) => void;
}

class TileWorkerManager {
  private worker: Worker | null = null;
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
      const { type, id } = response; // type과 id는 공통 속성이므로 안전하게 추출 가능

      const callback = this.callbacks.get(id);

      if (!callback) return;

      switch (type) {
        case 'progress':
          // 부분 업데이트 (지형 -> 건물 -> ...)
          // TS가 'progress' 타입임을 알기 때문에 .data 접근이 가능해집니다.
          if (response.type === 'progress' && response.data) {
            callback.onProgress?.(response.data);
          }
          break;
        case 'complete':
          // 최종 완료
          if (response.type === 'complete') {
            callback.resolve(response.data);
            this.callbacks.delete(id);
          }
          break;
        case 'error':
          // 에러 처리
          // TS가 'error' 타입임을 알기 때문에 .error 접근이 가능해집니다.
          if (response.type === 'error') {
            console.error('[WorkerManager] Error:', response.error);
            callback.reject(response.error);
            this.callbacks.delete(id);
          }
          break;
      }
    };

    this.worker.onerror = (err) => {
      console.error('[WorkerManager] Worker global error:', err);
    };
  }

  /**
   * 타일 생성 요청 (Promise 반환)
   */
  public requestTile(
    tx: number,
    tz: number,
    onProgress?: (data: Partial<WorldTile>) => void,
  ): Promise<WorldTile> {
    const id = `${tx}:${tz}`;

    // 워커가 혹시 죽었거나 없으면 재실행
    if (!this.worker) this.initWorker();

    return new Promise((resolve, reject) => {
      // 콜백 등록
      this.callbacks.set(id, { resolve, reject, onProgress });

      // 워커에게 작업 지시
      this.worker?.postMessage({ tx, tz, id });
    });
  }

  public terminate() {
    this.worker?.terminate();
    this.worker = null;
    this.callbacks.clear();
  }
}

// 싱글톤 인스턴스로 내보내기
export const tileWorkerManager = new TileWorkerManager();
