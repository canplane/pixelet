// // @pxl/engine/modules/page/service.ts

// import { IoDescriptor } from "@xpute/spec/io/descriptor.ts";
// import { PAGE_OP } from "./opcode.ts";
// import { PAGE_MANIFEST } from "./ext/manifest.ts";
// import { bake_elevation, bake_features } from "./baker.ts";

// // [Optimization] OpCode -> StreamDefinition 매핑 (O(1) Lookup)
// const _STREAM_LUT = new Map<number, any>();
// for (const layer of PAGE_MANIFEST.layers) {
//   for (const stream of layer.frames) {
//     _STREAM_LUT.set(stream.opCode, stream);
//   }
// }

// /**
//  * [Page Service]
//  * 순수 비즈니스 로직: Raw Binary + Manifest -> Baked Data
//  * 네트워크 의존성을 제거하여 테스트 용이성과 이식성을 확보했습니다.
//  */
// export const PageService = {
//   /**
//    * [bake]
//    * @param pageId 페이지 식별자
//    * @param opCode 스트림 식별자
//    * @param rawBuffer 입력 바이너리 (From Network/DB)
//    */
//   async bake(pageId: number, opCode: number, rawBuffer: Uint8Array): Promise<Uint8Array> {
//     // 1. [Manifest Lookup] 설계도 찾기
//     const streamDef = _STREAM_LUT.get(opCode);
//     if (!streamDef) {
//       console.warn(`[PageService] Unknown OpCode: 0x${opCode.toString(16)}. Drop.`);
//       return new Uint8Array(0);
//     }

//     // 2. [Binding] IoDescriptor에 설계도 주입
//     // DOD(Data-Oriented Design): 데이터 해석 시점에 스키마를 바인딩
//     const in_desc = new IoDescriptor(rawBuffer, streamDef);
//     in_desc.load();

//     // 3. [Process] OpCode에 따른 분기
//     if (opCode === PAGE_OP.TERRAIN) {
//       return this._bakeTerrain(in_desc);
//     } else if (opCode === PAGE_OP.POI) {
//       return this._bakePOI(in_desc);
//     }

//     return new Uint8Array(0);
//   },

//   // ---- Internal Logic ----

//   _bakeTerrain(desc: IoDescriptor): Uint8Array {
//     // Manifest에 정의된 'id'로 접근
//     const raw_ele = desc.read("ele");
//     const raw_land = desc.read("land");

//     // Baking Logic (CPU Heavy)
//     const elevation = raw_ele ? bake_elevation(raw_ele) : new Uint8Array(0);
//     const { ground, poi } = raw_land ? bake_features(raw_land) : { ground: { land: null, water: null }, poi: null };

//     // Result Assembly
//     const out_buf = new Uint8Array(5 * 1024 * 1024);
//     const out_desc = new IoDescriptor(out_buf); // Write Mode (No Schema needed for simple output)

//     if (elevation.length > 0) out_desc.write("ele", elevation);
//     if (ground.land) out_desc.write("land", ground.land);
//     if (ground.water) out_desc.write("water", ground.water);
//     if (poi) out_desc.write("poi", poi);

//     const writtenSize = out_desc.flush();
//     return out_buf.subarray(0, writtenSize);
//   },

//   _bakePOI(desc: IoDescriptor): Uint8Array {
//     const raw_labels = desc.read("labels");
//     return raw_labels || new Uint8Array(0);
//   },
// };
