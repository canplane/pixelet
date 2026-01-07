/**
 * @file sparse-quadtree-codec.ts
 * [Engine Core] Binary Codec for SparseQuadtree
 * * @summary
 * SparseQuadtree 인스턴스를 단일 ArrayBuffer로 직렬화(Encode)하거나
 * 바이너리 스트림에서 복원(Decode)하는 정적 유틸리티입니다.
 * * 🎯 [Refactor] STL 스타일의 __SparseQuadtree 구현체 대응 및 병렬 레이아웃 적용
 * * * * @format [Binary Layout]
 * [Header: 8 bytes]
 * - Offset 0: Size (Uint32) - 맵의 한 변의 길이
 * - Offset 4: Detail Count (Uint32) - 생성된 상세 블록(Mixed)의 총 개수
 * [Body 1: Macro Values (U<T>)]
 * - 🎯 [Uniform Data]: indices가 -1인 영역의 대표값들을 저장
 * - Length: (Size/64)^2 * BytesPerElement
 * [Body 2: Indices Structure (Int32)]
 * - 🎯 [Structure Map]: -1(Uniform) 또는 상세 블록 인덱스(>=0)를 저장
 * - Length: (Size/64)^2 * 4 bytes
 * [Body 3: Details (Heap)]
 * - 🎯 [Mixed Data]: 실제 상세 데이터 블록들의 연속된 나열
 * - Length: (Detail Count) * (4096 * BytesPerElement)
 */

import { __SparseQuadtree, SparseQuadtree } from './sparse-quadtree';
import { UniversalTypedArray as U, TypedArrayConstructor, TypedArray } from './types';

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const HEADER_SIZE = 8; // Size(4) + Count(4)
const BLOCK_LEN = 4096; // 64 * 64

export const SparseQuadtreeCodec = {
  /**
   * 💾 [Encode] to Binary Blob
   * 트리의 런타임 상태(U<T>)를 선형적인 메모리 레이아웃으로 직렬화합니다.
   * 🚀 [Efficiency] TypedArray.set()을 이용한 Zero-copy 지향 memcpy 수행
   */
  encode<T extends TypedArray>(tree: SparseQuadtree<T>): ArrayBuffer {
    // 🎯 [STL Pattern] 인터페이스를 통해 내부 __ 구현체의 속성에 접근
    const { size, macro, indices, details } = tree as unknown as __SparseQuadtree<T>;
    const detailCount = details.length;

    // 1. Calculate Buffer Size
    // 🚀 [Fix] (macro as any) 대신 constructor 참조로 정적 속성에 안전하게 접근합니다.
    const elementSize = (macro.constructor as any).BYTES_PER_ELEMENT;
    const macroSize = macro.byteLength;
    const indicesSize = indices.byteLength;
    const detailsSize = detailCount * BLOCK_LEN * elementSize;

    const totalSize = HEADER_SIZE + macroSize + indicesSize + detailsSize;
    const buffer = new ArrayBuffer(totalSize);
    const view = new DataView(buffer);

    // 2. Write Header (Little Endian)
    view.setUint32(0, size, true);
    view.setUint32(4, detailCount, true);

    // 3. Write Macro Grid (Values)
    // 🎯 [Memory] 원본 버퍼의 slice가 아닌 View를 통한 직접 복사
    new Uint8Array(buffer, HEADER_SIZE, macroSize).set(new Uint8Array(macro.buffer, macro.byteOffset, macro.byteLength));

    // 4. Write Indices (Structure)
    // Int32Array 뷰를 생성하여 고속 복사
    const indicesOffset = HEADER_SIZE + macroSize;
    new Int32Array(buffer, indicesOffset, indices.length).set(indices);

    // 5. Write Details (Heap Copy)
    if (detailCount > 0) {
      const detailsOffset = indicesOffset + indicesSize;
      const destBytes = new Uint8Array(buffer, detailsOffset);
      const blockSizeBytes = BLOCK_LEN * elementSize;

      for (let i = 0; i < detailCount; i++) {
        const srcBlock = details[i];
        const srcBytes = new Uint8Array(srcBlock.buffer, srcBlock.byteOffset, srcBlock.byteLength);
        destBytes.set(srcBytes, i * blockSizeBytes);
      }
    }

    return buffer;
  },

  /**
   * 📂 [Decode] from Binary Blob
   * 바이너리 스트림에서 U<T> 규격의 런타임 인스턴스를 복원합니다.
   */
  decode<T extends TypedArray>(buffer: ArrayBuffer, BlockCtor: TypedArrayConstructor<T>): SparseQuadtree<T> {
    const view = new DataView(buffer);

    // 1. Read Header
    const size = view.getUint32(0, true);
    const detailCount = view.getUint32(4, true);

    const gridCount = size >> 6;
    const mLen = gridCount * gridCount;
    const elementSize = BlockCtor.BYTES_PER_ELEMENT;

    // 2. Restore Macro Grid (Values)
    const macroOffset = HEADER_SIZE;
    const macroSize = mLen * elementSize;
    // 🎯 [Isolation] buffer.slice()를 통해 원본 버퍼와 독립된 메모리 블록 할당
    const macro = new BlockCtor(buffer.slice(macroOffset, macroOffset + macroSize));

    // 3. Restore Indices (Structure)
    const indicesOffset = macroOffset + macroSize;
    const indicesSize = mLen * 4;
    const indices = new Int32Array(buffer.slice(indicesOffset, indicesOffset + indicesSize));

    // 4. Restore Details (Heap Reconstruction)
    const details: U<T>[] = new Array(detailCount);
    const detailsStart = indicesOffset + indicesSize;
    const blockSizeBytes = BLOCK_LEN * elementSize;

    if (detailCount > 0) {
      for (let i = 0; i < detailCount; i++) {
        const offset = detailsStart + i * blockSizeBytes;
        const blockBuffer = buffer.slice(offset, offset + blockSizeBytes);
        details[i] = new BlockCtor(blockBuffer);
      }
    }

    /** 🚀 [The Shadowing]
     * 복원된 U<T> 데이터들을 __SparseQuadtree에 주입하고
     * 최종적으로 투명한 SparseQuadtree<T> 인터페이스로 반환합니다.
     */
    return new __SparseQuadtree<T>(size, BlockCtor, macro, indices, details) as unknown as SparseQuadtree<T>;
  },
};
