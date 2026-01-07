/**
 * @file json-codec.ts
 * [Shared Lib] JSON 규격에 맞는 데이터만 바이너리로 변환하며,
 * 규격 위반 시 즉시 예외를 발생시켜 시스템 오염을 차단합니다.
 */

export const JsonCodec = {
  /**
   * [Encode] 객체를 UTF-8 바이너리로 변환
   * @throws {Error} 순환 참조나 직렬화 불가능한 데이터 포함 시 발생
   */
  encode<T>(data: T): Uint8Array {
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
        throw new Error('[JsonCodec] Result is undefined. Input is not a valid JSON shape.');
      }

      return new TextEncoder().encode(json);
    } catch (e) {
      // 시스템 레벨의 에러로 래핑하여 리포팅
      throw new Error(`[JsonCodec] Encode failed: ${e instanceof Error ? e.message : 'Unknown error'}`);
    }
  },

  /**
   * [Decode] 바이너리를 객체로 복원
   */
  decode<T>(buffer: ArrayBuffer): T | null {
    if (!buffer || buffer.byteLength === 0) return null;

    try {
      const json = new TextDecoder().decode(buffer);
      const result = JSON.parse(json);

      // 복원 후 최소한의 형상 체크 (필요 시)
      if (result === undefined) return null;

      return result as T;
    } catch (e) {
      // Decode는 실패 시 로깅 후 null을 반환하여 시스템 가용성을 우선시합니다.
      console.error('[JsonCodec] Decode failed:', e);
      return null;
    }
  },
};
