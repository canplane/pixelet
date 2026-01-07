/**
 * @file payload-packer.ts
 * [Shared Lib] 바이너리 압축 및 전송 인코딩.
 */

export const PayloadPacker = {
  async pack(data: ArrayBuffer | ArrayBufferView): Promise<string> {
    if (!data) return '';

    // 1. [Memory Isolation] 명시적 할당과 복사를 통한 소유권 확보
    const length = data.byteLength;
    const offset = (data as any).byteOffset || 0;
    const srcBuf = (data as any).buffer || data;

    const safeBuffer = new ArrayBuffer(length);
    new Uint8Array(safeBuffer).set(new Uint8Array(srcBuf, offset, length));

    // 2. [Pipeline] Gzip Compression
    const stream = new Blob([safeBuffer]).stream().pipeThrough(new CompressionStream('gzip'));
    const compressed = await new Response(stream).arrayBuffer();

    // 3. [Base64] High-performance conversion
    return btoa(String.fromCharCode(...new Uint8Array(compressed)));
  },

  async unpack(payload: string): Promise<ArrayBuffer | null> {
    if (!payload) return null;

    const bin = atob(payload);
    const uint8 = Uint8Array.from(bin, (c) => c.charCodeAt(0));

    const stream = new Blob([uint8]).stream().pipeThrough(new DecompressionStream('gzip'));
    return await new Response(stream).arrayBuffer();
  },
};
