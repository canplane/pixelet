// @pxl/spec/cell/utils.ts
// 셀 처리를 위한 비트마스킹 상수와 유틸리티 함수입니다.

// =============================================================================
// Bitmask Constants
// =============================================================================

/** 🛡️ 엔진의 모든 상태 판별을 위한 물리적 기점 (Seed Value) */
export const ZERO = 0x00;

/** 🎭 8비트 데이터 무결성을 위한 풀 마스크 (All-On) */
export const FULL = 0xff;

export const CELL_MASKS = {
  /** 🔄 Sea Flag (Bit 7): 1이면 바다/심해 영역 (128) */
  SEA_FLAG: 0x80, // 1000 0000 (128)
  /** 🆔 ID Value Mask (Bit 0~6): 순수 재질 ID 추출 (127) */
  ID_VALUE: 0x7f, // 0111 1111 (127)
} as const;

// =============================================================================
// Utilities
// =============================================================================

// export const CELL_BITMASK = {
//   /**
//    * 해당 ID가 바다(Sea) 속성을 가지고 있는지 확인합니다.
//    * @param id Cell ID
//    */
//   is_sea: (id: number): boolean => (id & CELL_MASKS.SEA_FLAG) !== 0,

//   /**
//    * 해당 ID가 육지(Land) 속성을 가지고 있는지 확인합니다.
//    * @param id Cell ID
//    */
//   is_land: (id: number): boolean => (id & CELL_MASKS.SEA_FLAG) === 0,

//   /**
//    * 플래그를 제거한 순수 재질 ID를 반환합니다.
//    * Ex) SEA(129) -> BARE(1)
//    * @param id Cell ID
//    */
//   get_pure_id: (id: number): number => id & CELL_MASKS.ID_VALUE,

//   /**
//    * 해당 ID를 바다 속성으로 변환합니다.
//    * @param id Cell ID (e.g., BARE -> SEA)
//    */
//   to_sea: (id: number): number => id | CELL_MASKS.SEA_FLAG,

//   /**
//    * 해당 ID를 육지 속성으로 변환합니다.
//    * @param id Cell ID (e.g., SEA -> BARE)
//    */
//   to_land: (id: number): number => id & CELL_MASKS.ID_VALUE,
// };

// export const color_ops = {
//   /**
//    * 오직 6자리 HEX(#RRGGBB) 형식만 유효한 색상으로 간주합니다.
//    * 8자리(RGBA)나 null, 잘못된 문자열은 모두 투명 또는 무효 처리합니다.
//    * @param hex 검증할 HEX 문자열
//    */
//   is_valid_hex: (hex: string | null | undefined): hex is string => {
//     if (!hex) return false;
//     return /^#([A-Fa-f0-9]{6})$/.test(hex);
//   },
// };

// =============================================================================
// Semantic Status (New Augmentation)
// 🎯 [Architecture] 부등호 연산을 지양하고 셀의 시맨틱 상태를 판별합니다.
// 🚀 [Refactor] ZERO_VALUE와 비트 연산을 통해 숫자의 크기가 아닌 '상태'를 판독합니다.
// =============================================================================

// const RENDER_MASK = CELL_MASKS.ID_VALUE;

// export const cell_status = {
//   /** 💨 공기(0x00) 상태인지 확인합니다. (All-Off) */
//   is_air: (id: number): boolean => (id & FULL) === ZERO,

//   /** ❓ 정의되지 않은 구역(0xFF)인지 확인합니다. (All-On)
//    * 🎯 [Symmetry] ZERO_VALUE의 완벽한 비트 반전체인 FULL 그 자체입니다.
//    */
//   is_unknown: (id: number): boolean => (id & FULL) === FULL,

//   /**
//    * ✅ 렌더링이나 연산에 유효한 데이터(실체)인지 확인합니다.
//    * 🎯 [Bitwise] AIR(ZERO)도 아니고 UNKNOWN(FULL)도 아닌 모든 중간 상태를 의미합니다.
//    */
//   has_entity: (id: number): boolean => {
//     const masked = id & FULL;
//     // 🎯 [Minimal] 불필요한 변수 없이 원형 상수 2개로만 실체 판정
//     return masked !== ZERO && masked !== FULL;
//   },

//   /**
//    * 🎨 시각적 합성이 필요한 유효한 레이어인지 확인합니다.
//    * 🎯 [Symmetry] 순수 ID 영역이 비어있지(ZERO) 않고, 꽉 차있지(FULL) 않은지 비트로 판별합니다.
//    */
//   is_renderable: (id: number): boolean => {
//     const pure_id = id & RENDER_MASK;
//     return pure_id !== (ZERO & RENDER_MASK) && pure_id !== (FULL & RENDER_MASK);
//   },
// } as const;
