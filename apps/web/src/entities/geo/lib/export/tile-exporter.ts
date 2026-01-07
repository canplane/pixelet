/**
 * @file tile-exporter.ts
 * 엔진 내부의 타일 데이터(SparseQuadtree)를 시각적 이미지(PNG)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 * 🚀 [Final Evolution] ZERO/FULL 원자적 규격을 tf.js 연산에 직결하여 부등호를 완전히 소탕했습니다.
 *
 * [Refactor Note]
 * 1. 🎨 [Automation] BASE_COLORS를 순회하여 채널별 독립 색상 테이블을 자동 생성합니다. (유지보수 불필요)
 * 2. 🖌️ [Channeling] TILE_SURFACE_KEYS를 순회하며 모든 채널을 동적으로 합성(Painter's Algorithm)합니다.
 * 3. 🗺️ [Consistency] South-to-North (Y-Flip) 정책을 준수하여 월드 좌표계와 동기화합니다.
 * 4. ⚡ [Acceleration] 데이터 추출은 Quadtree(CPU)가, 색상 합성 및 리버스는 tf.js(GPU)가 담당합니다.
 *
 * [Engineering Guideline]
 * - 🛡️ No Chaining: tfjs 사용 시 타입 추론 오류 방지를 위해 모든 연산은 '함수형 API(tf.*)'로 통일합니다.
 * - 🛡️ Memory Safety: 모든 텐서 연산은 tf.tidy() 내에서 수행되어 GPU 메모리 누수를 방지합니다.
 */

import * as tf from '@tensorflow/tfjs-core';
import { BASE_TYPES, BASE_COLORS } from '@pixelet/core/entities/cell/model/base';
import { TILE_SURFACE_KEYS, type TileSurface, type TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { SparseQuadtree } from '@pixelet/core/shared/lib/structs/sparse-quadtree';
import { ZERO, FULL, CELL_MASKS } from '@pixelet/core/entities/cell/model/utils';
import { Chunk } from '@pixelet/core/entities/geo/model/chunks';
// import { ChunkHandler } from '@pixelet/core/entities/geo/logic/chunk/chunk-handler';

const { TILE_RES } = GEO_CONSTANTS;

export type ExportMode = 'base' | 'single' | 'combined' | 'elevation';

export interface ExportOptions {
  mode: ExportMode;
  targetChannel?: TileSurfaceKey;
  targetZ?: number;
}

/**
 * [createColorTable]
 * 🚀 [Automation] 복셀 타입별 정의된 색상을 GPU 매핑 테이블로 구축합니다.
 * 🎯 [Update] 채널별 독립된 ID 네임스페이스를 지원하기 위해 채널별 텐서 맵을 반환합니다.
 */
const createColorTable = () =>
  tf.tidy(() => {
    const lookup: Record<string, tf.Tensor> = {};
    Object.entries(BASE_COLORS).forEach(([channel, colors]) => {
      const channelKey = channel.toLowerCase();
      const colorArray = new Float32Array(256 * 3);

      Object.entries(colors).forEach(([id, hex]) => {
        // [Null-Safe Guard] AIR(null) 또는 잘못된 형식 방어
        if (!hex || typeof hex !== 'string' || !hex.startsWith('#')) return;

        const c = hex.slice(1);
        const i = Number(id) * 3;
        colorArray[i] = parseInt(c.slice(0, 2), 16) / 255;
        colorArray[i + 1] = parseInt(c.slice(2, 4), 16) / 255;
        colorArray[i + 2] = parseInt(c.slice(4, 6), 16) / 255;
      });
      lookup[channelKey] = tf.tensor2d(colorArray, [256, 3]);
    });
    return lookup;
  });

/**
 * [exportTileImage]
 * 🎯 [Refactor] ZERO/FULL 비트마스킹을 통해 부등호 없이 레이어를 합성합니다.
 */
export async function exportTileImage(
  surface: TileSurface,
  options: ExportOptions = { mode: 'base' },
  chunks?: Chunk[],
  elevation?: SparseQuadtree<Int16Array>,
): Promise<Blob | null> {
  const { mode, targetChannel, targetZ } = options;

  const rgbaTensor = tf.tidy(() => {
    const colors = createColorTable();

    /**
     * [Fallback Logic]
     * 모든 채널을 검사했는데도 데이터가 없다면(AIR/UNKNOWN),
     * 시각적 결손 방지를 위해 BASE_LAND 색상을 적용합니다.
     */
    const bareLandId = tf.scalar(BASE_TYPES.LAND.BARE, 'int32');
    const fallbackColor = tf.reshape(tf.gather(colors['land'], bareLandId), [1, 3]);

    let composite: tf.Tensor;

    /**
     * 🚀 [Semantic Masking with Shape Alignment]
     * ZERO/FULL 대칭성을 이용한 실체 판정.
     * 🎯 [Fix] 비트 연산 시 스칼라 값을 텐서로 명시하여 Shape Mismatch 에러를 해결합니다.
     */
    const getRenderableMask = (ids: tf.Tensor) => {
      const isNotAir = tf.notEqual(ids, ZERO); // 0(ZERO)이 아니면(Not Air) 그릴 실체가 있는 것으로 판정
      return tf.expandDims(isNotAir, 1); // [N] -> [N, 1] 확장하여 Painter's Algorithm 합성
    };

    // --- PHASE 1. Layer Composition ---
    if (mode === 'elevation' && elevation) {
      const denseEl = tf.tensor2d(new Float32Array(elevation.flatten()), [TILE_RES, TILE_RES]);
      const normalizedEl = tf.div(tf.clipByValue(denseEl, 0, 255), 255);
      composite = tf.reshape(tf.stack([normalizedEl, normalizedEl, normalizedEl], 2), [-1, 3]);
    } else {
      const channelTensors = TILE_SURFACE_KEYS.map((key) => {
        const tree = surface[key];
        // 🎯 [Optimization] 쿼드트리 데이터를 Int32 텐서로 변환하여 비트 연산 무결성 확보
        const flattened = tree ? new Int32Array(tree.flatten()) : new Int32Array(TILE_RES * TILE_RES);
        return tf.reshape(tf.tensor1d(flattened, 'int32'), [-1]);
      });

      /**
       * [Painter's Algorithm 정교화]
       * ZERO(AIR)로 시작하여 TILE_SURFACE_KEYS 순서대로 유효한 값을 덮어씌웁니다.
       */
      composite = tf.gather(colors['land'], channelTensors[0]);
      composite = tf.where(getRenderableMask(channelTensors[0]), composite, fallbackColor);

      for (let i = 1; i < channelTensors.length; i++) {
        const key = TILE_SURFACE_KEYS[i];
        if (mode === 'single' && targetChannel && key !== targetChannel) continue;
        const ids = channelTensors[i];
        // 🎯 핵심: 값(ids)뿐만 아니라 채널(key) 정보도 함께 사용하여 독립 색상 맵에서 색상을 찾습니다.
        composite = tf.where(getRenderableMask(ids), tf.gather(colors[key], ids), composite);
      }

      // // --- Combined Mode: Build Layer Overlay (Legacy Chunk Support) ---
      // if (mode === 'combined' && chunks) {
      //   for (const chunk of chunks) {
      //     const chunkHandler = new ChunkHandler(chunk.cells);
      //     const sliceData = new Int32Array(32 * 32);
      //     for (let cz = 0; cz < 32; cz++) {
      //       for (let cx = 0; cx < 32; cx++) {
      //         sliceData[cz * 32 + cx] = chunkHandler.get(cx, targetZ ?? 0, cz);
      //       }
      //     }
      //     /**
      //      * 🎯 [Sequential Write Simulation]
      //      * 텐서 pad와 reshape를 통해 캔버스 좌표에 데이터를 정렬하여 삽입합니다.
      //      */
      //     const padded = tf.pad(tf.tensor2d(sliceData, [32, 32], 'int32'), [
      //       [(chunk.coord.z % 32) * 32, TILE_RES - 32 - (chunk.coord.z % 32) * 32],
      //       [(chunk.coord.x % 32) * 32, TILE_RES - 32 - (chunk.coord.x % 32) * 32],
      //     ]);
      //     const paddedIds = tf.reshape(padded, [-1]);
      //     const buildMask = tf.expandDims(tf.notEqual(tf.bitwiseAnd(paddedIds, tf.scalar(FULL, 'int32')), tf.scalar(ZERO, 'int32')), 1);
      //     composite = tf.where(buildMask, tf.gather(colors['structure'], paddedIds), composite);
      //   }
      // }
    }

    // --- PHASE 2. World Alignment & Formatting ---
    const finalReshaped = tf.reshape(composite, [TILE_RES, TILE_RES, 3]);

    /**
     * [getGridIdx 명제 계승]
     * 캔버스의 y는 위(0)에서 아래(res-1)로 가지만,
     * 월드 데이터의 gy는 아래(0)에서 위(res-1)로 가므로 여기서 변환(Y-Flip)합니다.
     */
    const flipped = tf.reverse(finalReshaped, [0]);
    const alpha = tf.ones([TILE_RES, TILE_RES, 1]);

    // 🎯 [Optimization] mul(255)와 cast('int32')를 제거하여 정규화(0~1) 상태 유지
    return tf.concat([flipped, alpha], 2) as tf.Tensor3D;
  });

  // --- PHASE 3. IO Strategy ---
  try {
    const canvas = document.createElement('canvas');
    canvas.width = TILE_RES;
    canvas.height = TILE_RES;

    // 🚀 [GPU Direct Draw] toPixels 대신 draw를 사용하여 GPU 텍스처를 캔버스에 직접 복사
    await tf.browser.draw(rgbaTensor, canvas);

    tf.dispose(rgbaTensor);
    return new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
  } catch (e) {
    console.error('[TileExporter] Critical Failure:', e);
    if (rgbaTensor) tf.dispose(rgbaTensor);
    return null;
  }
}
