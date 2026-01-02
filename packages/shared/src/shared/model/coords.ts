/** [Coord4326] EPSG:4326 (WGS84) 위경도 좌표계 */
export type Coord4326 = {
  lng: number;
  lat: number;
  alt?: number; // 고도 (단위: meter, 해수면 기준)
};

/** * [Coord] EPSG:3857 (Web Mercator) 미터 좌표계 (우리 시스템 표준)
 * 지리적 절대 좌표 (단위: meter)
 * 💡 우리 프로젝트의 물리 법칙 표준입니다.
 * x: 동서 (East-West)
 * y: 남북 (North-South) -> 🎯 북쪽이 +Y
 * z: 고도 (Altitude)     -> 🎯 위쪽이 +Z
 */
export interface Coord {
  x: number;
  y: number;
  z: number;
}

/** * [Coord2D]
 * 평면 연산(충돌 검사, 타일 인덱싱 등)을 위한 2D 좌표계
 */
export type Coord2D = Omit<Coord, 'z'>;

// --- 변환 유틸리티 ---

/** 3D -> 2D (Z축 무시) */
export const to2D = (coord: Coord): Coord2D => ({
  x: coord.x,
  y: coord.y,
});

/** 2D -> 3D (Z축 주입) */
export const to3D = (coord: Coord2D, z: number = 0): Coord => ({
  ...coord,
  z,
});

/** * [CoordOps]
 * 좌표계 간의 산술 연산을 지원하는 순수 함수 집합입니다.
 */
export const CoordOps = {
  /** * [add] 벡터 합
   * 사용 예: CoordOps.add(poi.center, VOXEL_SIZE.HALF)
   */
  add: <T extends Coord2D | Coord>(base: T, offset: number | Partial<T>): T => {
    // 1. 숫자가 들어오면 x, y (z) 모두에 더함
    if (typeof offset === 'number') {
      return {
        ...base,
        x: base.x + offset,
        y: base.y + offset,
        ...('z' in base ? { z: (base as Coord).z + offset } : {}),
      } as T;
    }
    // 2. 객체가 들어오면 각 성분별로 더함 (Partial 지원)
    return {
      ...base,
      x: base.x + (offset.x ?? 0),
      y: base.y + (offset.y ?? 0),
      ...('z' in base && 'z' in (offset as any)
        ? { z: (base as Coord).z + ((offset as any).z ?? 0) }
        : {}),
    } as T;
  },

  /** [sub] 벡터 차 */
  sub: <T extends Coord2D | Coord>(base: T, offset: Partial<T>): T => {
    return {
      ...base,
      x: base.x - (offset.x ?? 0),
      y: base.y - (offset.y ?? 0),
      ...('z' in base && 'z' in (offset as any)
        ? { z: (base as Coord).z - ((offset as any).z ?? 0) }
        : {}),
    } as T;
  },

  /** [lerp] 선형 보간 (두 좌표 사이의 중간 지점 계산) */
  lerp: (a: Coord2D, b: Coord2D, t: number): Coord2D => ({
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
  }),
};
