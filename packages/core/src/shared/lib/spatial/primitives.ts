/**
 * @file primitives.ts
 * [Domain: Spatial] Primitive Coordinate Types
 * 의존성 없는 순수 타입 정의 파일입니다.
 */

// =============================================================================
// 1. Base Structures
// =============================================================================

export interface XY {
  x: number;
  y: number;
}
export interface XYZ extends XY {
  z: number;
}

/** [GeoPos] EPSG:4326 (WGS84) 위경도 좌표 (단위: Degree) */
export interface GeoPos {
  lng: number; // 경도 (x)
  lat: number; // 위도 (y)
  alt?: number; // 고도 (meters, optional)
}

// =============================================================================
// 2. Real Number Space (Meters) - 'Pos'
// =============================================================================

/** [Pos2] 2D 절대/상대 좌표 */
export type Pos2 = XY & { readonly __brand?: 'real' };

/** [Pos3] 3D 절대/상대 좌표 */
export type Pos3 = XYZ & { readonly __brand?: 'real' };

// =============================================================================
// 3. Integer Space (Grid Index) - 'Coord'
// =============================================================================

/** [Coord2] 2D 그리드 인덱스 */
export type Coord2 = XY & { readonly __brand?: 'int' };

/** [Coord3] 3D 그리드 인덱스 */
export type Coord3 = XYZ & { readonly __brand?: 'int' };

// =============================================================================
// 4. Casting Helpers (Type Assertion only)
// ⚠️ 주의: 이 함수들은 값을 반올림(round)하거나 보정하지 않습니다.
// 단순히 타입 시스템을 만족시키기 위한 캐스팅(Assertion) 함수입니다.
// =============================================================================

/** (Unsafe) Raw Object -> Pos2 Casting (No Runtime Cost) */
export const asPos2 = (v: XY): Pos2 => v as Pos2;
export const asPos3 = (v: XYZ): Pos3 => v as Pos3;

/** (Unsafe) Raw Object -> Coord2 Casting (No Rounding/Flooring) */
export const asCoord2 = (v: XY): Coord2 => v as Coord2;
export const asCoord3 = (v: XYZ): Coord3 => v as Coord3;

// =============================================================================
// 5. Dimension Helpers (Structural Conversion)
// =============================================================================

/**
 * [flatten] 3D -> 2D (Drop Z)
 * z축을 버리고 평면 좌표만 남깁니다.
 */
export const flatten = (v: XYZ): Pos2 => ({ x: v.x, y: v.y });

/**
 * [expand] 2D -> 3D (Inject Z)
 * 평면 좌표에 높이(z)를 부여하여 3D 좌표로 확장합니다.
 */
export const expand = (v: XY, z: number = 0): Pos3 => ({ x: v.x, y: v.y, z });

// =============================================================================
// 6. Bridge Helpers (Array <-> Object)
// =============================================================================

export const toArray2 = (v: XY): [number, number] => [v.x, v.y];
export const fromArray2 = (arr: ArrayLike<number>): Pos2 => ({ x: arr[0], y: arr[1] }) as Pos2;

export const toArray3 = (v: XYZ): [number, number, number] => [v.x, v.y, v.z];
export const fromArray3 = (arr: ArrayLike<number>): Pos3 => ({ x: arr[0], y: arr[1], z: arr[2] }) as Pos3;
