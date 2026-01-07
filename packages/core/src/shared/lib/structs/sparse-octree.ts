/**
 * @file sparse-octree.ts
 * [Engine Core] Generic 2-Level Sparse Octree (Volume)
 * * @summary
 * 3D 공간 데이터(Voxel, Density)를 위한 하드웨어 친화적 공간 분할 자료구조입니다.
 * 64x64x64(2^6) 블록 단위를 기준으로 매크로(L1)와 디테일(L2) 계층을 분리하여 관리합니다.
 * STL의 vector나 span처럼 메모리 버퍼(View)와 접근자(Accessor) 기능만 제공합니다.
 * * @architecture
 * 1. [L1 Macro Values]: (size/64)^3 그리드 (U<T>).
 * - [Uniform Mode]: 대응하는 indices 값이 -1일 때, 해당 큐브 영역 전체를 대표하는 단일 값(Value)을 저장.
 * 2. [L1 Indices]: (size/64)^3 그리드 (Int32Array).
 * - 🎯 [Structure Controller]: 3D 블록의 상태와 데이터 위치를 결정하는 마스터 계층.
 * - 값 == -1: [Uniform] 해당 영역은 L1 Macro의 값을 사용함.
 * - 값 >= 0 : [Mixed] 해당 영역은 L2 Details 배열의 특정 인덱스(Block)를 참조함.
 * 3. [L2 Details]: 64x64x64 실제 데이터 블록들의 힙(Heap). 🎯 [Sparse]: 변경이 발생한 영역만 동적 할당.
 * 4. [Access]: 비트 연산 및 3D Stride 계산을 통한 O(1) Random Access 보장.
 */

import { UniversalTypedArray as U, TypedArrayConstructor, TypedArray } from './types';

// -----------------------------------------------------------------------------
// Constants (Structural Optimization)
// -----------------------------------------------------------------------------

// 64x64x64 Block Size hardcoded for Bitwise Optimization
const BLOCK_SHIFT = 6;
const BLOCK_MASK = 63;
const BLOCK_RES = 1 << BLOCK_SHIFT; // 64
const BLOCK_AREA = BLOCK_RES * BLOCK_RES; // 4096 (2D slice)
const BLOCK_VOL = BLOCK_AREA * BLOCK_RES; // 262,144 (3D volume)

// -----------------------------------------------------------------------------
// Class Definition
// -----------------------------------------------------------------------------

/**
 * 🛠️ [Internal Implementation]
 * STL의 std::basic_string과 같은 역할입니다.
 */
export class __SparseOctree<T extends TypedArray> {
  /**
   * 🔒 [Raw Data] L1 Macro Values (U<T>)
   * 🎯 [Generalization]: 데이터 타입 T와 일치하는 유니버설 뷰를 사용합니다.
   */
  public readonly macro: U<T>;

  /**
   * 🔒 [Raw Structure] L1 Structure Indices (Int32)
   * 🎯 [Symmetry]: 값과 구조를 분리하여 3D 인덱싱의 무결성을 보장합니다.
   */
  public readonly indices: Int32Array;

  /**
   * 🔒 [Raw Data] L2 Detailed Blocks
   * - Sparse하게 할당된 64^3 블록들의 리스트
   */
  public readonly details: U<T>[];

  /**
   * 📏 [Dimension] 전체 맵의 한 변의 길이 (Power of 2 권장)
   */
  public readonly size: number;

  /**
   * 🧮 [Dimension] Macro Grid의 한 변의 개수 (size / 64)
   */
  private readonly gridCount: number;

  /**
   * 🏭 [Factory Reference]
   * 새로운 상세 블록 할당(Mutation) 시 사용할 생성자
   */
  private readonly BlockCtor: TypedArrayConstructor<T>;

  /**
   * @constructor
   * 🚀 [Universal Mapping]: 주입된 데이터를 ArrayBufferLike 호환형(U<T>)으로 매핑하여 초기화합니다.
   */
  constructor(size: number, BlockCtor: TypedArrayConstructor<T>, macro?: U<T>, indices?: Int32Array, details?: U<T>[]) {
    // Validation
    if (size & BLOCK_MASK) {
      throw new Error(`[SparseOctree] Size must be a multiple of ${BLOCK_RES}. Given: ${size}`);
    }

    this.size = size;
    this.gridCount = size >> BLOCK_SHIFT;
    this.BlockCtor = BlockCtor;

    const expectedMacroLen = this.gridCount * this.gridCount * this.gridCount;

    this.macro = macro ?? new BlockCtor(expectedMacroLen);
    this.indices = indices ?? new Int32Array(expectedMacroLen).fill(-1);
    this.details = details ?? [];

    // Optional: Validate Macro Buffer Size
    if (this.macro.length < expectedMacroLen) {
      console.warn(`[SparseOctree] Macro buffer too small. Expected: ${expectedMacroLen}, Got: ${this.macro.length}`);
    }
  }

  // ===========================================================================
  // ⚡️ High-Performance Accessors (Hot Path)
  // ===========================================================================

  /**
   * 🔍 [Get] O(1) 3D Random Access
   */
  public get(x: number, y: number, z: number): number {
    const bx = x >> BLOCK_SHIFT;
    const by = y >> BLOCK_SHIFT;
    const bz = z >> BLOCK_SHIFT;

    const gridSq = this.gridCount * this.gridCount;
    const macroIdx = bz * gridSq + by * this.gridCount + bx;

    // 🎯 [Parallel Logic] 인덱스 버퍼를 통해 구조를 먼저 파악
    const detailIdx = this.indices[macroIdx];

    // Branch 1: Uniform Block
    if (detailIdx === -1) {
      return this.macro[macroIdx];
    }

    // Branch 2: Mixed Block -> Resolve L2 Index
    const lx = x & BLOCK_MASK;
    const ly = y & BLOCK_MASK;
    const lz = z & BLOCK_MASK;

    return this.details[detailIdx][lz * BLOCK_AREA + ly * BLOCK_RES + lx];
  }

  /**
   * ✏️ [Set] Sparse Allocation Strategy
   */
  public set(x: number, y: number, z: number, value: number): void {
    const bx = x >> BLOCK_SHIFT;
    const by = y >> BLOCK_SHIFT;
    const bz = z >> BLOCK_SHIFT;

    const gridSq = this.gridCount * this.gridCount;
    const macroIdx = bz * gridSq + by * this.gridCount + bx;
    const detailIdx = this.indices[macroIdx];

    // Case 1: 이미 Mixed 상태
    if (detailIdx >= 0) {
      const lx = x & BLOCK_MASK;
      const ly = y & BLOCK_MASK;
      const lz = z & BLOCK_MASK;
      this.details[detailIdx][lz * BLOCK_AREA + ly * BLOCK_RES + lx] = value;
      return;
    }

    // Case 2: Uniform 상태이며 값이 동일함
    if (this.macro[macroIdx] === value) return;

    // Case 3: Uniform -> Mixed 변환 (Auto Split)
    this._splitBlock(macroIdx, this.macro[macroIdx], x, y, z, value);
  }

  /**
   * 🧹 [Compact] Merge Optimization
   */
  public compact(): void {
    for (let i = 0; i < this.indices.length; i++) {
      const detailIdx = this.indices[i];
      if (detailIdx >= 0) {
        this._tryMergeBlock(i, detailIdx);
      }
    }
  }

  /**
   * 📦 [Flatten] To Dense Array
   */
  public flatten(): U<T> {
    const fullSize = this.size * this.size * this.size;
    const result = new this.BlockCtor(fullSize);
    const gridSq = this.gridCount * this.gridCount;

    for (let bz = 0; bz < this.gridCount; bz++) {
      for (let by = 0; by < this.gridCount; by++) {
        for (let bx = 0; bx < this.gridCount; bx++) {
          const macroIdx = bz * gridSq + by * this.gridCount + bx;
          const detailIdx = this.indices[macroIdx];
          const destBase = bz * BLOCK_RES * this.size * this.size + by * BLOCK_RES * this.size + bx * BLOCK_RES;

          if (detailIdx === -1) {
            this._fillRegion(result, destBase, this.macro[macroIdx]);
          } else {
            this._copyBlock(result, destBase, this.details[detailIdx]);
          }
        }
      }
    }
    return result;
  }

  // ===========================================================================
  // 🔒 Internals (Encapsulated Logic)
  // ===========================================================================

  private _splitBlock(macroIdx: number, fillVal: number, x: number, y: number, z: number, newVal: number): void {
    const newBlock = new this.BlockCtor(BLOCK_VOL);
    newBlock.fill(fillVal);

    const lx = x & BLOCK_MASK;
    const ly = y & BLOCK_MASK;
    const lz = z & BLOCK_MASK;
    newBlock[lz * BLOCK_AREA + ly * BLOCK_RES + lx] = newVal;

    const newDetailIdx = this.details.push(newBlock) - 1;
    this.indices[macroIdx] = newDetailIdx;
  }

  private _tryMergeBlock(macroIdx: number, detailIdx: number): void {
    const block = this.details[detailIdx];
    const firstVal = block[0];

    for (let i = 1; i < BLOCK_VOL; i++) {
      if (block[i] !== firstVal) return;
    }

    this.indices[macroIdx] = -1;
    this.macro[macroIdx] = firstVal;
  }

  private _fillRegion(target: U<T>, startOffset: number, val: number): void {
    const sliceStride = this.size * this.size;
    const rowStride = this.size;

    for (let z = 0; z < BLOCK_RES; z++) {
      const sliceStart = startOffset + z * sliceStride;
      for (let y = 0; y < BLOCK_RES; y++) {
        const rowStart = sliceStart + y * rowStride;
        target.fill(val, rowStart, rowStart + BLOCK_RES);
      }
    }
  }

  private _copyBlock(target: U<T>, startOffset: number, source: U<T>): void {
    const sliceStride = this.size * this.size;
    const rowStride = this.size;

    for (let z = 0; z < BLOCK_RES; z++) {
      const destSliceStart = startOffset + z * sliceStride;
      const srcSliceStart = z * BLOCK_AREA;

      for (let y = 0; y < BLOCK_RES; y++) {
        const destIdx = destSliceStart + y * rowStride;
        const srcIdx = srcSliceStart + (y << BLOCK_SHIFT);
        target.set(source.subarray(srcIdx, srcIdx + BLOCK_RES), destIdx);
      }
    }
  }
}

/** 🌲 [Standard Interface] */
export type SparseOctree<T extends TypedArray> = __SparseOctree<U<T>>;

/** 🏭 [Public Constructor] (Value Shadowing) */
export const SparseOctree = __SparseOctree as unknown as {
  new <T extends TypedArray>(size: number, BlockCtor: TypedArrayConstructor<T>, macro?: U<T>, indices?: Int32Array, details?: U<T>[]): SparseOctree<T>;
};
