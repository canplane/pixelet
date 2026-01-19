// // @pxl/web/modules/world/terrain/export/tile-binary.ts

// /**
//  * @file tile-binary.ts
//  * [Layer: GEO/Codec] 페이지 데이터의 최종 바이너리 직렬화를 담당하는 Facade 인터페이스입니다.
//  * * [Architecture: Facade]
//  * - 이 파일은 고수준의 페이지 객체(SparseQuadtree)를 저수준의 전송 규격(Uint8Array)으로 변환하는 단일 창구 역할을 합니다.
//  * - 내부적으로는 SparseQuadtreeCodec을 사용하여 최적화된 메모리 레이아웃을 생성하며,
//  * 외부 레이어(Service, Exporter 등)가 내부 코덱의 상세 구현에 의존하지 않도록 격리합니다.
//  */

// import type { TypedArray } from "@xpute/spec/lib/kit/array.ts";

// /**
//  * [generateTileBinary]
//  * SparseQuadtree 인스턴스를 네트워크 전송 및 저장에 최적화된 표준 바이너리 버퍼로 직렬화합니다.
//  *
//  * [Binary Layout Specification]
//  * - 기반 기술: SparseQuadtreeCodec (Header 8b + Macro + Details)
//  * - 정렬 방식: Little-Endian (System Standard)
//  * - 반환 형식: Uint8Array (Stream-ready)
//  *
//  * @param tree - 직렬화할 Sparse 구조의 런타임 데이터 인스턴스
//  * @returns 전송 규격에 맞춰 패킹된 바이너리 데이터
//  */
// export function generateTileBinary<T extends TypedArray>(tree: SparseQuadtree<T>): Uint8Array {
//   // 🚀 [Facade] 내부 코덱의 ArrayBuffer 결과물을 Uint8Array 뷰로 래핑하여
//   // 상위 레이어에서 즉시 처리(전송/저장) 가능한 형태로 추상화합니다.
//   const dense = tree.flatten(); // TypedArray
//   return new Uint8Array(dense.buffer, dense.byteOffset, dense.byteLength);
// }
