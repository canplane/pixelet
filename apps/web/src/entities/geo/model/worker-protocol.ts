/**
 * @file worker-protocol.ts
 * 메인 스레드와 타일 워커 간의 통신 규격을 정의합니다.
 */

import type { ServiceMode } from '@/config/app-config';
import type { TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';

export interface TileWorkerInput {
  key: string;
  // 1. 공간 좌표 정보 (어디를 그릴 것인가)
  coord: TileCoord;
  // 2. 실행 및 환경 옵션 (어떻게 그릴 것인가)
  options: {
    serviceMode: ServiceMode;
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
  data?: any; // Tile 또는 partialTile
  error?: string;
}
