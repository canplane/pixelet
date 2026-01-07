/**
 * @file sparse-octree-codec.ts
 * [Engine Core] Binary Codec for SparseOctree
 * * @summary
 * SparseOctree 인스턴스를 단일 ArrayBuffer로 직렬화(Encode)하거나
 * 바이너리 스트림에서 복원(Decode)하는 정적 유틸리티입니다.
 * * 🎯 [Refactor] STL 스타일의 __SparseOctree 구현체 대응 및 병렬 레이아웃 적용
 * * * * @format [Binary Layout]
 * [Header: 8 bytes]
 * - Offset 0: Size (Uint32) - 맵의 한 변의 길이
 * - Offset 4: Detail Count (Uint32) - 생성된 상세 블록의 개수
 * [Body 1: Macro Values (U<T>)]
 * - 🎯 [Uniform Data]: indices가 -1인 큐브 영역의 대표값들을 저장
 * - Length: (Size/64)^3 * BytesPerElement
 * [Body 2: Indices Structure (Int32)]
 * - 🎯 [Structure Map]: -1(Uniform) 또는 상세 블록 인덱스(>=0)를 저장
 * - Length: (Size/64)^3 * 4 bytes
 * [Body 3: Details (Heap)]
 * - 🎯 [Mixed Data]: 실제 상세 데이터 블록(64^3)들의 연속된 나열
 * - Length: (Detail Count) * (262144 * BytesPerElement)
 */

import { __SparseOctree, SparseOctree } from './sparse-octree';
import { UniversalTypedArray as U, TypedArrayConstructor, TypedArray } from './types';

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const HEADER_SIZE = 8; // Size(4) + Count(4)
const BLOCK_VOL = 262144; // 64 * 64 * 64

export const SparseOctreeCodec = {
  /**
   * 💾 [Encode] to Binary Blob
   * 트리의 런타임 상태(U<T>)를 3D 선형 메모리 레이아웃으로 직렬화합니다.
   */
  encode<T extends TypedArray>(tree: SparseOctree<T>): ArrayBuffer {
    // 🎯 STL 스타일: 인터페이스를 통해 내부 구현체 속성에 접근
    const { size, macro, indices, details } = tree as unknown as __SparseOctree<T>;
    const detailCount = details.length;

    // 1. Calculate Buffer Size
    const elementSize = (macro.constructor as any).BYTES_PER_ELEMENT;
    const macroSize = macro.byteLength;
    const indicesSize = indices.byteLength;
    const detailsSize = detailCount * BLOCK_VOL * elementSize;

    const totalSize = HEADER_SIZE + macroSize + indicesSize + detailsSize;
    const buffer = new ArrayBuffer(totalSize);
    const view = new DataView(buffer);

    // 2. Write Header
    view.setUint32(0, size, true);
    view.setUint32(4, detailCount, true);

    // 3. Write Macro Grid (Values)
    new Uint8Array(buffer, HEADER_SIZE, macroSize).set(new Uint8Array(macro.buffer, macro.byteOffset, macro.byteLength));

    // 4. Write Indices (Structure)
    const indicesOffset = HEADER_SIZE + macroSize;
    new Int32Array(buffer, indicesOffset, indices.length).set(indices);

    // 5. Write Details (Heap Copy)
    if (detailCount > 0) {
      const detailsOffset = indicesOffset + indicesSize;
      const destBytes = new Uint8Array(buffer, detailsOffset);
      const blockSizeBytes = BLOCK_VOL * elementSize;

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
   * 바이너리 스트림에서 U<T> 규격의 3D 런타임 인스턴스를 복원합니다.
   */
  decode<T extends TypedArray>(buffer: ArrayBuffer, BlockCtor: TypedArrayConstructor<T>): SparseOctree<T> {
    const view = new DataView(buffer);

    // 1. Read Header
    const size = view.getUint32(0, true);
    const detailCount = view.getUint32(4, true);

    const gridCount = size >> 6;
    const mLen = gridCount * gridCount * gridCount; // 3D Macro Count
    const elementSize = BlockCtor.BYTES_PER_ELEMENT;

    // 2. Restore Macro Grid (Values)
    const macroOffset = HEADER_SIZE;
    const macroSize = mLen * elementSize;
    const macro = new BlockCtor(buffer.slice(macroOffset, macroOffset + macroSize));

    // 3. Restore Indices (Structure)
    const indicesOffset = macroOffset + macroSize;
    const indicesSize = mLen * 4;
    const indices = new Int32Array(buffer.slice(indicesOffset, indicesOffset + indicesSize));

    // 4. Restore Details (Heap Reconstruction)
    const details: U<T>[] = new Array(detailCount);
    const detailsStart = indicesOffset + indicesSize;
    const blockSizeBytes = BLOCK_VOL * elementSize;

    if (detailCount > 0) {
      for (let i = 0; i < detailCount; i++) {
        const offset = detailsStart + i * blockSizeBytes;
        const blockBuffer = buffer.slice(offset, offset + blockSizeBytes);
        details[i] = new BlockCtor(blockBuffer);
      }
    }

    /** 🚀 [The Shadowing]
     * 내부 구현체를 생성하되 표준 인터페이스로 단언하여 반환
     */
    return new __SparseOctree<T>(size, BlockCtor, macro, indices, details) as unknown as SparseOctree<T>;
  },
};
