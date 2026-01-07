/**
 * @file sparse-quadtree.ts
 * [Engine Core] Generic 2-Level Sparse Quadtree
 * * @summary
 * 2D 평면 데이터를 위한 하드웨어 친화적 공간 분할 자료구조입니다.
 * 64x64(2^6) 블록 단위를 기준으로 매크로(L1)와 디테일(L2) 계층을 분리하여 관리합니다.
 * STL의 vector나 span처럼 메모리 버퍼(View)와 접근자(Accessor) 기능만 제공합니다.
 * * @architecture
 * 1. [L1 Macro Values]: (size/64) x (size/64) 그리드 (U<T>).
 * - [Uniform Mode]: 대응하는 indices 값이 -1일 때, 해당 영역 전체를 대표하는 단일 값(Value)을 저장.
 * 2. [L1 Indices]: (size/64) x (size/64) 그리드 (Int32Array).
 * - 🎯 [Structure Controller]: 블록의 상태와 데이터 위치를 결정하는 마스터 계층.
 * - 값 == -1: [Uniform] 해당 영역은 L1 Macro의 값을 사용함.
 * - 값 >= 0 : [Mixed] 해당 영역은 L2 Details 배열의 특정 인덱스(Block)를 참조함.
 * 3. [L2 Details]: 64x64 실제 데이터 블록들의 힙(Heap). 🎯 [Sparse]: 변경이 발생한 영역만 동적 할당.
 * 4. [Access]: 비트 연산(Bitwise) 및 인덱스 점프를 통한 O(1) Random Access 보장.
 */

import { UniversalTypedArray as U, TypedArrayConstructor, TypedArray } from './types';

// -----------------------------------------------------------------------------
// Constants (Structural Optimization)
// -----------------------------------------------------------------------------

// 64x64 Block Size is hardcoded for Bitwise Optimization (>> 6, & 63)
const BLOCK_SHIFT = 6;
const BLOCK_MASK = 63;
const BLOCK_RES = 1 << BLOCK_SHIFT; // 64
const BLOCK_SIZE = BLOCK_RES * BLOCK_RES; // 4096

// -----------------------------------------------------------------------------
// Class Definition
// -----------------------------------------------------------------------------

/**
 * 🛠️ [Internal Implementation]
 * STL의 std::basic_string과 같은 역할입니다.
 * 코덱이나 엔진 코어 로직에서 직접 접근할 수 있도록 export합니다.
 */
export class __SparseQuadtree<T extends TypedArray> {
  /**
   * 🔒 [Raw Data] L1 Macro Values (U<T>)
   * Uniform 블록일 경우 해당 영역의 대표 수치를, Mixed일 경우 캐시된 대표값을 보관합니다.
   */
  public readonly macro: U<T>;

  /**
   * 🔒 [Raw Structure] L1 Structure Indices (Int32)
   * 🎯 [Symmetry]: 값(Macro)과 구조(Indices)를 분리하여 데이터 타입 T에 관계없이 일관된 공간 인덱싱을 제공합니다.
   */
  public readonly indices: Int32Array;

  /**
   * 🔒 [Raw Data] L2 Detailed Blocks
   * - Sparse하게 할당된 64x64 블록들의 리스트
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
   * 🚀 [Universal Mapping]: 주입된 TypedArray를 ArrayBufferLike 호환형(U<T>)으로 매핑하여 초기화합니다.
   */
  constructor(size: number, BlockCtor: TypedArrayConstructor<T>, macro?: U<T>, indices?: Int32Array, details?: U<T>[]) {
    // Validation
    if (size & BLOCK_MASK) {
      throw new Error(`[SparseQuadtree] Size must be a multiple of ${BLOCK_RES}. Given: ${size}`);
    }

    this.size = size;
    this.gridCount = size >> BLOCK_SHIFT;
    this.BlockCtor = BlockCtor;

    const expectedMacroLen = this.gridCount * this.gridCount;

    // 🎯 [Generalization] 데이터 타입에 맞는 macro와 구조 전용 indices 초기화
    this.macro = macro ?? new BlockCtor(expectedMacroLen);
    this.indices = indices ?? new Int32Array(expectedMacroLen).fill(-1);
    this.details = details ?? [];

    // Optional: Validate Macro Buffer Size
    if (this.macro.length < expectedMacroLen) {
      console.warn(`[SparseQuadtree] Macro buffer too small. Expected: ${expectedMacroLen}, Got: ${this.macro.length}`);
    }
  }

  // ===========================================================================
  // ⚡️ High-Performance Accessors (Hot Path)
  // ===========================================================================

  /**
   * 🔍 [Get] O(1) Random Access
   * 좌표 (x, y)의 값을 조회합니다.
   */
  public get(x: number, y: number): number {
    // L1 Indexing (x / 64)
    const bx = x >> BLOCK_SHIFT;
    const by = y >> BLOCK_SHIFT;
    const macroIdx = by * this.gridCount + bx;

    // 🎯 [Parallel Logic] 인덱스 버퍼를 통해 구조를 먼저 파악
    const detailIdx = this.indices[macroIdx];

    // Branch 1: Uniform Block (Fastest)
    if (detailIdx === -1) {
      return this.macro[macroIdx];
    }

    // Branch 2: Mixed Block -> Resolve L2 Index
    const lx = x & BLOCK_MASK;
    const ly = y & BLOCK_MASK;

    return this.details[detailIdx][(ly << BLOCK_SHIFT) + lx];
  }

  /**
   * ✏️ [Set] Sparse Allocation Strategy
   * 좌표 (x, y)에 값을 설정하며, 필요 시 자동으로 메모리를 할당(Expand)합니다.
   */
  public set(x: number, y: number, value: number): void {
    const bx = x >> BLOCK_SHIFT;
    const by = y >> BLOCK_SHIFT;
    const macroIdx = by * this.gridCount + bx;

    const detailIdx = this.indices[macroIdx];

    // Case 1: 이미 Mixed 상태 (Fast Path)
    if (detailIdx >= 0) {
      const lx = x & BLOCK_MASK;
      const ly = y & BLOCK_MASK;
      this.details[detailIdx][(ly << BLOCK_SHIFT) + lx] = value;
      return;
    }

    // Case 2: Uniform 상태이며 값이 동일함 (Skip)
    if (this.macro[macroIdx] === value) return;

    // Case 3: Uniform -> Mixed 변환 (Auto Split)
    this._splitBlock(macroIdx, this.macro[macroIdx], x, y, value);
  }

  /**
   * 🧹 [Compact] Merge Optimization
   * 전체 트리를 검사하여, 모든 값이 동일한 상세 블록을 찾아 Macro(Uniform) 상태로 되돌립니다.
   * 🎯 [Boilerplate Cut] 베이커와의 정합성을 위해 명칭을 compact로 통일.
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
   * 🎯 [Full Reconstruction]: Sparse 구조를 해제하고 선형적인 메모리 레이아웃으로 복원합니다.
   */
  public flatten(): U<T> {
    const fullSize = this.size * this.size;
    const result = new this.BlockCtor(fullSize);

    for (let by = 0; by < this.gridCount; by++) {
      for (let bx = 0; bx < this.gridCount; bx++) {
        const macroIdx = by * this.gridCount + bx;
        const detailIdx = this.indices[macroIdx];
        const destBase = by * BLOCK_RES * this.size + bx * BLOCK_RES;

        if (detailIdx === -1) {
          // Fill Uniform Area
          this._fillRegion(result, destBase, this.macro[macroIdx]);
        } else {
          // Copy Mixed Block
          this._copyBlock(result, destBase, this.details[detailIdx]);
        }
      }
    }
    return result;
  }

  // ===========================================================================
  // 🔒 Internals (Encapsulated Logic)
  // ===========================================================================

  /**
   * [Internal] Split (Expand)
   * 🎯 [Promotion]: Uniform(Value-based) 영역을 Mixed(Block-based) 영역으로 승격시킵니다.
   */
  private _splitBlock(macroIdx: number, fillVal: number, x: number, y: number, newVal: number): void {
    const newBlock = new this.BlockCtor(BLOCK_SIZE);
    newBlock.fill(fillVal);

    const lx = x & BLOCK_MASK;
    const ly = y & BLOCK_MASK;
    newBlock[(ly << BLOCK_SHIFT) + lx] = newVal;

    const newDetailIdx = this.details.push(newBlock) - 1;
    // 🎯 [Parallel Logic] 인덱스 버퍼 업데이트
    this.indices[macroIdx] = newDetailIdx;
  }

  /**
   * [Internal] Merge (Collapse)
   * 🎯 [Demotion]: 모든 값이 동일한 상세 블록을 해제하고 다시 Uniform 상태로 환원합니다.
   */
  private _tryMergeBlock(macroIdx: number, detailIdx: number): void {
    const block = this.details[detailIdx];
    const firstVal = block[0];

    // 1. Check Uniformity (O(4096))
    for (let i = 1; i < BLOCK_SIZE; i++) {
      if (block[i] !== firstVal) return; // 하나라도 다르면 병합 불가
    }

    // 2. Merge Success: Update Structure
    this.indices[macroIdx] = -1; // Mark as Uniform
    this.macro[macroIdx] = firstVal; // Store Value

    // 3. Release Memory (Lazy Release)
  }

  private _fillRegion(target: U<T>, startOffset: number, val: number): void {
    const stride = this.size;
    for (let y = 0; y < BLOCK_RES; y++) {
      const rowStart = startOffset + y * stride;
      target.fill(val, rowStart, rowStart + BLOCK_RES);
    }
  }

  private _copyBlock(target: U<T>, startOffset: number, source: U<T>): void {
    const stride = this.size;
    for (let y = 0; y < BLOCK_RES; y++) {
      const destIdx = startOffset + y * stride;
      const srcIdx = y << BLOCK_SHIFT;
      target.set(source.subarray(srcIdx, srcIdx + BLOCK_RES), destIdx);
    }
  }
}

/**
 * 🌲 [Standard Interface]
 * C++의 std::string과 같은 별칭입니다.
 * 🎯 [The Shadowing]: 밖에서 넘기는 모든 TypedArray를 자동으로 공유 메모리 호환형(U<T>)으로 승격시킵니다.
 */
export type SparseQuadtree<T extends TypedArray> = __SparseQuadtree<U<T>>;

/**
 * 🏭 [Public Constructor] (Value Shadowing)
 * 🎯 [Factory Interface]: 사용자는 __ 구현체나 U<T> 승격 로직을 몰라도 표준 생성자만으로 인스턴스를 제어할 수 있습니다.
 */
export const SparseQuadtree = __SparseQuadtree as unknown as {
  new <T extends TypedArray>(size: number, BlockCtor: TypedArrayConstructor<T>, macro?: U<T>, indices?: Int32Array, details?: U<T>[]): SparseQuadtree<T>;
};
