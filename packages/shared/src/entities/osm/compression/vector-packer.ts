/**
 * @file vector-packer.ts
 * [General Purpose Packer]
 * 복잡한 바이너리 오프셋 관리 없이, JSON + Gzip 조합으로 유연성과 압축률을 동시에 확보합니다.
 * 어떤 객체 배열(T[])이든 처리 가능합니다.
 */

import { decompressBase64 } from './compression-utils';

/**
 * [packVector] - Server Side
 * 임의의 객체 배열을 JSON 문자열로 변환 후 Gzip 압축합니다.
 * * @param data 압축할 데이터 배열 (예: MapPoi[])
 * @param gzipSync Node.js의 zlib.gzipSync 함수 주입
 */
export const packVector = <T>(
  data: T[],
  gzipSync: (buf: Uint8Array, opt?: any) => any,
): string => {
  // 1. 데이터가 없으면 빈 문자열 반환 (오버헤드 방지)
  if (!data || data.length === 0) return '';

  // 2. JSON 직렬화
  const jsonString = JSON.stringify(data);

  // 3. String -> Uint8Array (TextEncoder는 Node/Browser 공통 표준)
  const uint8 = new TextEncoder().encode(jsonString);

  // 4. Gzip 압축 (Server Side)
  const compressed = gzipSync(uint8, { level: 9 });

  // 5. Base64 인코딩
  return (compressed as any).toString('base64');
};

/**
 * [unpackVector] - Client Side
 * 압축된 Base64 문자열을 해제하여 원본 객체 배열로 복원합니다.
 * * @template T 복원할 데이터 타입 (예: MapPoi)
 */
export const unpackVector = async <T>(b64: string): Promise<T[]> => {
  // 1. 데이터가 없으면 빈 배열 반환
  if (!b64 || b64.length === 0) return [];

  // 2. Base64 -> Gzip Decompress -> Uint8Array
  const decompressed = await decompressBase64(b64);

  // 3. Uint8Array -> String
  const jsonString = new TextDecoder().decode(decompressed);

  // 4. JSON 파싱
  return JSON.parse(jsonString) as T[];
};
