// // @pxl/common/lib/common-utils.ts

// /**
//  * 프로젝트 전반에서 사용되는 원시적(Primitive) 성격의 공통 유틸리티 함수 모음입니다.
//  * 특정 도메인 로직을 포함하지 않으며, 순수 함수(Pure Function) 위주로 구성됩니다.
//  */

// // =============================================================================
// // [1] Naming Utilities (네이밍 관련)
// // =============================================================================

// /**
//  * 중복되지 않는 이름을 생성하여 반환합니다.
//  * 주어진 이름(baseName)이 이미 존재할 경우, 뒤에 숫자를 붙여(예: "Name 1", "Name 2") 고유성을 확보합니다.
//  * * @param baseName - 기본이 되는 이름
//  * @param namePool - 이미 존재하는 이름들의 배열
//  * @returns 중복되지 않는 고유한 이름 문자열
//  */
// export const makeUniqueName = (
//   baseName: string,
//   namePool: string[],
// ): string => {
//   const nameSet = new Set(namePool);

//   if (!nameSet.has(baseName)) {
//     return baseName;
//   }

//   let name = baseName;
//   // 무한 루프 방지 및 안전한 넘버링을 위해 1부터 순차 탐색
//   for (let i = 1;; i++) {
//     name = `${baseName} ${i}`;
//     if (!nameSet.has(name)) {
//       return name;
//     }
//   }
// };

// // =============================================================================
// // [2] Type Guards & Validation (타입 검사 및 유효성 확인)
// // =============================================================================

// /**
//  * 값이 순수 객체(Plain Object)인지 확인합니다.
//  * **주의:** 자바스크립트에서 배열도 'object'이지만, 이 함수는 **배열을 제외한 객체**만 true로 반환합니다.
//  */
// export const isObject = (val: unknown): val is Record<string, unknown> => {
//   return val !== null && typeof val === "object" && Array.isArray(val);
// };

// /**
//  * 문자열이 유효한 숫자 형식을 가지는지 확인합니다.
//  * 소수점으로 끝나는 경우(예: "123.")는 불완전한 숫자로 간주하여 false를 반환합니다.
//  */
// export const isNumberString = (s: string): boolean => {
//   if (!s || s.trim() === "") return false;
//   if (s.slice(-1) === ".") return false; // "12." 같은 입력 방지
//   return !Number.isNaN(Number(s)) && !Number.isNaN(parseFloat(s));
// };

// // =============================================================================
// // [3] Data Structure Manipulation (자료구조 조작)
// // =============================================================================

// /**
//  * Java의 `Map.getOrDefault` 혹은 Python의 `defaultdict`와 유사한 기능을 수행합니다.
//  * Map에서 키에 해당하는 값을 찾고, 없으면 기본값(defaultValue)을 세팅한 후 반환합니다.
//  * * @param map - 대상 Map 객체
//  * @param key - 조회할 키
//  * @param defaultValue - 키가 없을 경우 설정하고 반환할 기본값
//  */
// export const getOrDefaultMap = <K, V>(
//   map: Map<K, V>,
//   key: K,
//   defaultValue: V,
// ): V => {
//   let value = map.get(key);
//   if (value === undefined) {
//     value = defaultValue;
//     map.set(key, value);
//   }
//   return value;
// };

// // =============================================================================
// // [4] Functional Object Helpers (객체를 위한 함수형 유틸)
// // =============================================================================

// export function isJSONNumberString(s: string): boolean {
//   return /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/.test(s);
// }

// /**
//  * Object의 각 엔트리에 대해 함수를 실행합니다. (Array.prototype.forEach와 유사)
//  */
// export const forEachObject = <V>(
//   obj: Record<string, V>,
//   fn: (value: V, key: string) => void,
// ): void => {
//   Object.entries(obj).forEach(([k, v]) => fn(v, k));
// };

// /**
//  * Object의 각 값을 변환하여 새로운 Object를 생성합니다. (Array.prototype.map과 유사)
//  * 키(Key)는 유지되고 값(Value)만 변환됩니다.
//  */
// export const mapObject = <V, T>(
//   obj: Record<string, V>,
//   fn: (value: V, key: string) => T,
// ): Record<string, T> => {
//   return Object.fromEntries(Object.entries(obj).map(([k, v]) => [k, fn(v, k)]));
// };

// /**
//  * 조건(predicate)을 만족하는 엔트리만 남긴 새로운 Object를 반환합니다. (Array.prototype.filter와 유사)
//  */
// export const filterObject = <V>(
//   obj: Record<string, V>,
//   fn: (value: V, key: string) => boolean,
// ): Record<string, V> => {
//   return Object.fromEntries(Object.entries(obj).filter(([k, v]) => fn(v, k)));
// };
