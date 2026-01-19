// @xpute/spec/io/byte_envelope.ts

/**
 * Compress
 * 압축(Gzip), 스트림 처리.
 * 대용량 에셋(Map Chunk) 전송 시 packet을 감싸는 용도.
 */

import type { u32, u8 } from "@xpute/spec/lib/kit/word.ts";
import { as_bytes, type Bytes } from "@xpute/spec/lib/kit/array.ts";

/**
 * [Internal System Utility]
 * 바이너리 압축 및 전송 인코딩
 *
 * RULE
 * ----
 * - AB-backed Bytes면 추가 복사 없이 진행.
 * - SAB-backed면 as_bytes()가 AB로 강제 copy하여 안전.
 * - 반환은 Bytes(Uint8Array AB-backed)로 통일.
 */

/**
 * 📦 [Pack] -> Gzipped Bytes
 */
export async function pack(data: ArrayBuffer | ArrayBufferView): Promise<Bytes> {
  // normalize (SAB이면 여기서 AB로 copy)
  const buf: Bytes = as_bytes(data);
  if (buf.byteLength === 0) return new Uint8Array(0) as Bytes;

  // gzip
  const stream = new Blob([buf]).stream().pipeThrough(new CompressionStream("gzip"));
  const res = await new Response(stream).arrayBuffer();
  return new Uint8Array(res) as Bytes;
}

/**
 * 📂 [Unpack] Gzipped -> Bytes
 */
export async function unpack(data: Uint8Array): Promise<Bytes> {
  const buf: Bytes = as_bytes(data);
  if (buf.byteLength === 0) return new Uint8Array(0);

  const stream = new Blob([buf]).stream().pipeThrough(new DecompressionStream("gzip"));
  const res = await new Response(stream).arrayBuffer();
  return new Uint8Array(res);
}

/**
 * 📦 [PackIfLarge]
 * threshold 이상이면 gzip, 아니면 raw 유지.
 *
 * 반환: [payload, is_gz]
 * - is_gz=1이면 payload는 gzip bytes
 * - is_gz=0이면 payload는 원본 bytes(as_bytes)
 */
export async function pack_if_large(data: ArrayBuffer | ArrayBufferView, thres: u32 = 16 * 1024): Promise<[Bytes, u8]> {
  const buf: Bytes = as_bytes(data);
  if (buf.byteLength < thres) return [buf, 0];
  const gz = await pack(buf);
  return [gz, 1];
}
