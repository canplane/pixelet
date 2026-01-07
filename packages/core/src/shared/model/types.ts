/**
 * @file types.ts
 * [Engine Core] Primitive Coordinate Types
 *
 * * [Type Safety Strategy: Branded Types]
 * TypeScript의 구조적 타이핑(Duck Typing)으로 인한 실수(Meter)와 정수(Index)의 혼용을 막기 위해
 * 'Branded Type' 기법을 사용합니다. 런타임 오버헤드는 0입니다.
 *
 * * [Naming Convention]
 * - Pos  : Real Number (Float), Meters (연속적 공간) -> { x, y, z }
 * - Coord: Integer, Grid Index (이산적 공간) -> { x, y, z }
 */

// =============================================================================
// 1. Base Structures (For Destructuring & Library Compatibility)
// =============================================================================

/** 기본 2D 구조체 (x, y) */
export interface XY {
  x: number;
  y: number;
}

/** 기본 3D 구조체 (x, y, z) */
export interface XYZ extends XY {
  z: number;
}

// =============================================================================
// 2. Real Number Space (Position / Vector / Meter)
// =============================================================================

/**
 * [Pos2] 2D 절대/상대 좌표 (Meters)
 * - 사용처: WorldPos(w), LocalPos(l)
 */
export type Pos2 = XY & { readonly __brand?: 'real' };

/**
 * [Pos3] 3D 절대/상대 좌표 (Meters)
 * - 사용처: WorldPos(w), LocalPos(l)
 */
export type Pos3 = XYZ & { readonly __brand?: 'real' };

// =============================================================================
// 3. Integer Space (Grid Index / Coordinate / Offset)
// =============================================================================

/**
 * [Coord2] 2D 그리드 인덱스 (Integers)
 * - 사용처: TileCoord(t), CellCoord(o)
 */
export type Coord2 = XY & { readonly __brand?: 'int' };

/**
 * [Coord3] 3D 그리드 인덱스 (Integers)
 * - 사용처: ChunkCoord(c), CellCoord(o)
 */
export type Coord3 = XYZ & { readonly __brand?: 'int' };

// =============================================================================
// 4. Type Guards & Casters (Helper)
// =============================================================================

/** (Unsafe) 임의의 {x,y} 객체를 Pos2로 캐스팅합니다. */
export const asPos2 = (v: XY): Pos2 => v as Pos2;
export const asPos3 = (v: XYZ): Pos3 => v as Pos3;

/** (Unsafe) 임의의 {x,y} 객체를 Coord2로 캐스팅합니다. (반올림 없음, 단순 타입 변환) */
export const asCoord2 = (v: XY): Coord2 => v as Coord2;
export const asCoord3 = (v: XYZ): Coord3 => v as Coord3;

// --- Bridge Helpers (for gl-matrix/tf) ---

/** Pos3({x,y,z}) -> Array([x,y,z]) 변환 (gl-matrix 호환용) */
export const toArray3 = (v: XYZ): [number, number, number] => [v.x, v.y, v.z];

/** Array([x,y,z]) -> Pos3({x,y,z}) 변환 */
export const fromArray3 = (arr: ArrayLike<number>): Pos3 =>
  ({
    x: arr[0],
    y: arr[1],
    z: arr[2],
  }) as Pos3;

/**
 * 팁: gl-matrix를 쓸 때는 'xy'나 'xyz' 같은 swizzling보다
 * 명시적으로 변환 함수를 쓰는 것이 'Engine -> View' 경계를 넘는다는 것을
 * 코드에서 드러내기에 좋습니다.
 */
