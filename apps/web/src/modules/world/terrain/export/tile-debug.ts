// // @pxl/web/modules/world/terrain/export/tile-debug.ts

// /**
//  * South-to-North 페이지 데이터를 인간이 읽기 편하도록(North is Top) 콘솔에 출력합니다.
//  * 🚀 [Refactor] Sparse Grid(SparseQuadtree) 구조를 직접 탐색하며, tf.js 의존성 및 텐서 변환 오버헤드를 제거합니다.
//  */

// import { PAGE_CELL_SPAN } from "@pxl/bake/terrain/constants.ts";
// import type { SparseQuadtree } from "@pxl/spec/space/sparse-quadtree.ts";
// import type { TypedArray } from "@xpute/spec/lib/kit/array.ts";

// type PrintOptions = {
//   viewSize?: number; // 콘솔에 출력할 실제 크기 (너무 크면 잘리므로 기본 32)
//   offsetX?: number; // 샘플링 시작 X
//   offsetY?: number; // 샘플링 시작 Y
// };

// /**
//  * [printTileGrid]
//  * Sparse 데이터를 직접 탐색하여 북쪽이 위로 가도록 콘솔에 출력합니다.
//  * 🎯 [Engineering Note] 1024x1024 전체 출력은 콘솔 오버플로우를 유발하므로 viewSize로 제한합니다.
//  * 💡 [Dense Output] 소스는 Sparse하지만, 출력은 모든 좌표를 순회하며 점('.')으로 표현하여 공간적 가독성을 유지합니다.
//  */
// export function printTileGrid<T extends TypedArray>(tree: SparseQuadtree<T>, options: PrintOptions = {}) {
//   const { viewSize = 32, offsetX = 0, offsetY = 0 } = options;

//   // 1. [Metadata Info]
//   // 텐서 슬라이싱 대신 쿼드트리의 get() 메서드를 사용하여 필요한 부분만 즉시 샘플링합니다.
//   console.log(`\n[Grid Debug] ${viewSize}x${viewSize} View (Total ${PAGE_CELL_SPAN}x${PAGE_CELL_SPAN}, Top is North)`);
//   console.log(`[Offset] X:${offsetX}, Y:${offsetY}`);

//   // 2. [Symmetric Loop] North is Top (y축 역순 루프)
//   // 출력 범위: offsetY + viewSize - 1  --->  offsetY
//   for (let ly = viewSize - 1; ly >= 0; ly--) {
//     const gy = offsetY + ly; // 실제 로컬 y 좌표

//     // 페이지 해상도 경계 검사
//     if (gy >= PAGE_CELL_SPAN || gy < 0) continue;

//     let rowStr = `[Y:${gy.toString().padStart(4, " ")}] `;

//     for (let lx = 0; lx < viewSize; lx++) {
//       const gx = offsetX + lx; // 실제 로컬 x 좌표

//       if (gx >= PAGE_CELL_SPAN || gx < 0) {
//         rowStr += "    ";
//         continue;
//       }

//       // 🎯 [Direct Access] SparseQuadtree에서 해당 좌표의 값을 직접 획득 (O(log N))
//       // 데이터가 없는 블록(Sparse)은 쿼드트리 내부에서 즉시 0을 반환합니다.
//       const val = tree.get(gx, gy);

//       // 💡 [Dense Rendering] 값이 0인 곳은 '.'으로 표현하여 격자 구조를 유지합니다.
//       rowStr += val === 0 ? "  .  " : val.toString(16).padStart(4, "0").slice(-3) + " ";
//     }

//     // 3. [Console Flush] 한 행씩 출력
//     console.log(rowStr);
//   }
// }
