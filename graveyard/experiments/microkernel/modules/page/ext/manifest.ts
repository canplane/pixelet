// // @pxl/engine/src/modules/page/ext/manifest.ts

// import type { ResourceManifest } from "@xpute/spec/io/manifest.ts";
// import { PAGE_OP } from "../opcode.ts";

// /**
//  * [Engine Manifest]
//  * PXL 엔진이 정의한 'Terrain Layer' v1 스펙입니다.
//  * 이 객체는 IoDescriptor가 바이너리를 해석할 때 '설계도' 역할을 합니다.
//  */
// export const PAGE_MANIFEST: ResourceManifest = {
//   version: "pvf3-v1",
//   layers: [
//     {
//       id: "base",
//       // strategy: 'BLOCK', // Client Kernel은 이걸 보고 HTTP 요청을 날림
//       // endpoint: 'page/terrain/all',
//       // priority: 'CRITICAL',
//       frames: [
//         {
//           id: "terrain",
//           opCode: PAGE_OP.TERRAIN, // 0x31 (Mapping Key)
//           channels: [
//             // IoDescriptor는 이 순서와 타입(i16, u8)을 보고 메모리 오프셋을 계산합니다.
//             { id: "ele", type: "i16", layout: "DENSE" },
//             { id: "land", type: "u8", layout: "HYPERTREE" },
//             { id: "water", type: "u8", layout: "HYPERTREE" },
//             { id: "structure", type: "u8", layout: "HYPERTREE" },
//             { id: "transport", type: "u8", layout: "HYPERTREE" },
//           ],
//         },
//         {
//           id: "poi",
//           opCode: PAGE_OP.POI, // 0x32
//           channels: [
//             { id: "labels", type: "u8", layout: "DENSE" }, // DENSE but variable length logic internal
//           ],
//         },
//       ],
//     },
//   ],
// };
