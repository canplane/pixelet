// // @pxl/spec/cell/object.ts
// // [Layer: BUILD] Chunk 단위로 로딩되는 3D 사용자 데이터

// import { common_colors, common_types } from "./common.ts";

// // -----------------------------------------------------------------------------
// // ID Definitions
// // -----------------------------------------------------------------------------

// export const object_types = {
//   ...common_types, // 0 is RESERVED for AIR/ERASER in the parent scope

//   // ---- [BASIC MATERIALS] ----

//   /** ⬜ 기본 블록 (White) */
//   SOLID: 1,
//   /** 🧱 벽돌 */
//   BRICK: 2,
//   /** 🪵 나무 */
//   WOOD: 3,
//   /** 🪟 유리 */
//   GLASS: 4,
//   /** 🏢 콘크리트 */
//   CONCRETE: 5,

//   // ---- [SPECIAL] ----

//   /** 💡 조명 */
//   LIGHT: 10,
// } as const;

// // -----------------------------------------------------------------------------
// // Color Definitions
// // -----------------------------------------------------------------------------

// export const object_colors = {
//   ...common_colors,

//   // ---- [BASIC MATERIALS] ----

//   [object_types.SOLID]: "#FFFFFF",
//   [object_types.BRICK]: "#C62828",
//   [object_types.WOOD]: "#8D6E63",
//   [object_types.GLASS]: "#90CAF9", // Alpha should be handled in shader/renderer
//   [object_types.CONCRETE]: "#9E9E9E",

//   // ---- [SPECIAL] ----

//   [object_types.LIGHT]: "#FFF176",
// } as const;
