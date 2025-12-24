/**
 * [tile-storage-api]
 * 서버에 저장된 바이너리 타일 비트맵(.bin) 데이터를 로드하고 데이터 구조를 파싱합니다.
 */

/**
 * [fetch-binary-tile]
 * 타일의 원점 좌표를 기반으로 바이너리 파일을 요청하고 헤더 정보를 추출하여 반환합니다.
 */
export async function fetchBinaryTile(origin: { x: number; z: number }) {
  try {
    // 타일 좌표 기반의 파일 경로 (예: /api/tiles/1200_3400.bin)
    const response = await fetch(`/api/tiles/${origin.x}_${origin.z}.bin`);
    if (!response.ok) return null;

    const buffer = await response.arrayBuffer();
    const view = new DataView(buffer);

    /**
     * [parse-binary-header]
     * 바이너리 헤더 구조 파싱 (총 12바이트)
     * - 0~3: resolution (Int32)
     * - 4~7: originX (Int32)
     * - 8~11: originZ (Int32)
     * * 주의: 모든 정수는 Little Endian(true)으로 읽어야 합니다.
     */
    const resolution = view.getInt32(0, true);
    const originX = view.getInt32(4, true);
    const originZ = view.getInt32(8, true);

    /**
     * [extract-pixel-data]
     * 헤더(12바이트) 이후의 실제 비트맵 픽셀 데이터를 Uint8Array로 추출합니다.
     */
    const data = new Uint8Array(buffer, 12);

    return {
      data,
      resolution,
      origin: { x: originX, z: originZ },
    };
  } catch (error) {
    console.error(
      `[TileStorage] Failed to load binary tile at ${origin.x}, ${origin.z}:`,
      error,
    );
    return null;
  }
}
