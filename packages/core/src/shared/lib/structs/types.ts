/**
 * @file types.ts
 * [Engine Core] Memory Primitives Shadowing
 * 🚀 [Ultimate Fix] 표준 TypedArray 명칭을 탈취하여 전 프로젝트에 유니버설 규격을 강제합니다.
 */

/** 1. [Native Reference] 원본 타입 참조용 헬퍼 */
type NativeTypedArray = Int8Array | Uint8Array | Uint8ClampedArray | Int16Array | Uint16Array | Int32Array | Uint32Array | Float32Array | Float64Array;

/** * 2. [ToUniversalTypedArray]
 * 🎯 입력받은 타입을 Universal(ArrayBufferLike) 규격으로 승격시키는 로직.
 * 향후 BigInt64Array 등이 추가될 경우 이 섹션에 케이스를 추가합니다.
 */
export type ToUniversalTypedArray<T> = T extends Int8Array
  ? Int8Array<ArrayBufferLike>
  : T extends Uint8Array
    ? Uint8Array<ArrayBufferLike>
    : T extends Uint8ClampedArray
      ? Uint8ClampedArray<ArrayBufferLike>
      : T extends Int16Array
        ? Int16Array<ArrayBufferLike>
        : T extends Uint16Array
          ? Uint16Array<ArrayBufferLike>
          : T extends Int32Array
            ? Int32Array<ArrayBufferLike>
            : T extends Uint32Array
              ? Uint32Array<ArrayBufferLike>
              : T extends Float32Array
                ? Float32Array<ArrayBufferLike>
                : T extends Float64Array
                  ? Float64Array<ArrayBufferLike>
                  : T;

/** * 3. [Shadowing]
 * 🎯 이제 엔진 전반에서 'TypedArray'라고 쓰면 무조건 ArrayBufferLike 호환형입니다.
 */
export type TypedArray = ToUniversalTypedArray<NativeTypedArray>;

/** * 4. [Constructor Shadowing]
 * 표준 생성자 명칭을 유니버설 규격으로 재정의합니다.
 */
export type TypedArrayConstructor<T extends NativeTypedArray> = {
  new (length: number): ToUniversalTypedArray<T>;
  new (buffer: ArrayBufferLike, byteOffset?: number, length?: number): ToUniversalTypedArray<T>;
  new (elements: ArrayLike<number>): ToUniversalTypedArray<T>;
  readonly BYTES_PER_ELEMENT: number;
};

/** 🚀 [Sugar] 프로젝트 전반에서 U<T>로 짧게 사용하기 위한 별칭 */
export type UniversalTypedArray<T> = ToUniversalTypedArray<T>;
