// @pixelet/shared/io/json_codec.ts

/**
 * json codec
 * - encode: strict (throws on invalid JSON shape / cycles / bigint)
 * - decode: best-effort (logs and returns undefined on failure)
 */

import type { Bytes } from "@pixelet/shared/kit/array.ts";
import * as encoding from "@pixelet/shared/kit/encoding.ts";

/**
 * [Encode] 객체를 UTF-8 바이너리로 변환
 * @throws {Error} 순환 참조나 직렬화 불가능한 데이터 포함 시 발생
 */
export function encode<T>(data: T): Bytes {
  // 1. 데이터 부재 시 빈 바이너리 반환
  if (data === undefined || data === null) return new Uint8Array(0);

  try {
    // 2. [Strict Serialization]
    // JSON.stringify는 런타임에 가장 강력한 검사기입니다.
    // - 순환 참조 발견 시 Error 던짐
    // - BigInt 포함 시 Error 던짐
    const json = JSON.stringify(data);

    // 3. [Validation]
    // 만약 결과가 undefined라면 (함수나 symbol만 넣었을 경우) 예외 처리
    if (json === undefined) {
      throw new Error("json codec: result is undefined. input is not a valid json shape.");
    }

    return encoding.te.encode(json);
  } catch (err) {
    // 시스템 레벨의 에러로 래핑하여 리포팅
    throw new Error(`json codec: encode failed: ${err instanceof Error ? err.message : "unknown error"}`);
  }
}

/**
 * [Decode] 바이너리를 객체로 복원
 */
export function decode<T>(buffer: ArrayBuffer | ArrayBufferView): T | undefined {
  if (!buffer || buffer.byteLength === 0) return undefined;

  try {
    const json = encoding.td.decode(buffer);
    const result = JSON.parse(json);

    // 복원 후 최소한의 형상 체크 (필요 시)
    if (result === undefined) return undefined;

    return result as T;
  } catch (err) {
    console.error("json codec: decode failed:", err);
    return undefined;
  }
}
