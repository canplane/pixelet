// packages/shared/src/shared/lib/crypto-utils.ts

/**
 * [generate-hash]
 * 문자열 데이터를 SHA-256 알고리즘을 사용하여 고정 길이 해시로 변환합니다.
 * 주로 대규모 쿼리 문자열의 캐시 키 최적화에 사용됩니다.
 */
export async function generateHash(message: string): Promise<string> {
  const msgUint8 = new TextEncoder().encode(message);
  const hashBuffer = await crypto.subtle.digest('SHA-256', msgUint8);

  // Buffer를 Hex String으로 변환
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
}
