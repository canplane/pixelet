/**
 * @file grid-debug.ts
 * [lib/export/grid-debug.ts]
 * South-to-North 그리드 데이터를 인간이 읽기 편하도록(North is Top) 콘솔에 출력합니다.
 */

import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';

const { TILE_RES } = GEO_CONSTANTS;

/**
 * [printGrid]
 * 월드 좌표계 순서의 데이터를 뒤집어 출력하여 콘솔 상단이 북쪽(North)이 되도록 합니다.
 */
export function printGrid(
  grid: Uint8Array | Uint16Array,
  res: number = TILE_RES,
) {
  // 🎯 [Engineering Note]
  // 내부 데이터 순서는 남->북(0->res)이지만,
  // 콘솔은 위에서 아래로 출력되므로 루프를 역순(res->0)으로 돕니다.

  console.log(`\n[Grid Debug] ${res}x${res} (Top is North)`);

  for (let gy = res - 1; gy >= 0; gy--) {
    let rowStr = `[Y:${gy.toString().padStart(4, ' ')}] `;

    for (let gx = 0; gx < res; gx++) {
      const idx = gy * res + gx;
      const val = grid[idx];

      // 0은 빈 공간(.), 나머지는 16진수 표기
      rowStr += val === 0 ? ' . ' : val.toString(16).padStart(2, '0') + ' ';
    }
    console.log(rowStr);
  }
}
