/**
 * @file worker-types.ts
 * 메인 스레드와 타일 워커 간의 통신 규격을 정의합니다.
 */

export type ApiMode = 'local' | 'public';

export interface TileWorkerInput {
  // 1. 공간 좌표 정보 (어디를 그릴 것인가)
  coord: {
    tx: number;
    ty: number;
  };
  // 2. 실행 및 환경 옵션 (어떻게 그릴 것인가)
  options: {
    apiMode: ApiMode;
  };
  // 3. 추적 및 관리를 위한 메타 정보
  meta: {
    requestId: string;
    priority?: number;
  };
}

// 워커가 메인 스레드로 보내는 응답 타입도 정의하면 좋습니다.
export type WorkerMessageType = 'progress' | 'complete' | 'error';

export interface TileWorkerResponse {
  type: WorkerMessageType;
  requestId: string;
  data?: any; // WorldTile 또는 partialTile
  error?: string;
}
