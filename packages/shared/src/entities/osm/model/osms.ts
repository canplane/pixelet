/**
 * @file osms.ts
 * OSM 원본 데이터로부터 정제된 개별 객체 모델들을 정의합니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';

export interface ShapePath {
  points: MeterPos[];
  isClosed: boolean;
  holes?: MeterPos[][];
}

export interface MapInfra {
  id: string;
  type: 'building' | 'road' | 'water' | 'landuse';
  points: MeterPos[];
  center: MeterPos;
  shapePath?: ShapePath;
  height: number;
  tags: Record<string, string | undefined>;
}

export interface MapPoi {
  id: string;
  type: 'poi';
  category: string;
  name: string;
  center: MeterPos;
  tags: Record<string, string | undefined>;
}
