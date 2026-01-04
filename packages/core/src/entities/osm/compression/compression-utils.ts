/**
 * @file compression-utils.ts
 * 플랫폼 중립적인 압축/해제 유틸리티입니다.
 */

/**
 * [Client] Base64 -> Gzip Decompress -> Uint8Array
 */
export const decompressBase64 = async (b64: string): Promise<Uint8Array> => {
  if (!b64 || b64.length === 0) return new Uint8Array(0);

  const binaryString = atob(b64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }

  // DecompressionStream은 최신 브라우저 및 Node 18+ 지원
  const stream = new Blob([bytes])
    .stream()
    .pipeThrough(new DecompressionStream('gzip'));
  const response = new Response(stream);
  const buffer = await response.arrayBuffer();

  return new Uint8Array(buffer);
};
