/**
 * @file tf.ts
 * [Adapter] Spatial Primitives <-> TensorFlow.js
 * 텐서 연산을 위한 데이터 준비 헬퍼입니다.
 */
import type { Pos3, XYZ } from './primitives';

/**
 * [toTensorInput]
 * Pos3 객체를 tf.tensor()에 넣기 좋은 형태로 변환합니다.
 * 배치 처리를 위해 number[][] 형태가 필요할 때 유용합니다.
 */
export const toTensorInput = (v: XYZ): number[] => [v.x, v.y, v.z];

/**
 * [batchToTensorInput]
 * Pos3 배열을 [N, 3] 형태의 2D 배열로 변환합니다.
 */
export const batchToTensorInput = (vectors: XYZ[]): number[][] => vectors.map((v) => [v.x, v.y, v.z]);
