import { useMapStore } from '@/entities/osm/model/map-store';
import { cn } from '@/shared/lib/utils';
import { triggerDownload } from '@/shared/lib/io/browser-io';
import { generateGridBinary } from '@pixelet/shared/entities/osm/lib/io/grid-data';
import {
  exportGridImage,
  exportMergedGridImage,
} from '@/entities/osm/lib/export/tile-exporter';
import type { GridLayers } from '@pixelet/shared/entities/osm/model/tiles';

/**
 * [map-debug-tools]
 * 활성화된 모든 월드 타일의 '멀티 채널' 그리드 데이터를 추출합니다.
 */
export function MapDebugTools() {
  const tiles = useMapStore((state) => state.tiles);

  if (tiles.size === 0) return null;

  /**
   * 1. 바이너리 다운로드 핸들러
   */
  const handleDownloadAllBinary = () => {
    for (const [key, tile] of tiles) {
      const { grids } = tile;
      const { layers } = grids;

      // 채널들을 순회하며 각각 다운로드
      for (const channel of Object.keys(layers) as Array<keyof GridLayers>) {
        // 1. 바이너리 데이터 생성 (헤더 포함)
        const binaryData = generateGridBinary(layers[channel]);

        // 2. 다운로드 실행
        // 파일명: tile_10:5_base.bin
        triggerDownload(binaryData, `tile_${key}_${channel}.bin`);
      }
    }
  };

  /**
   * 2. 레이어별 이미지 프리뷰 핸들러
   */
  const handleDownloadAllImages = () => {
    // Promise.all을 사용하여 모든 타일의 처리를 병렬로 진행할 수 있습니다.
    for (const [key, tile] of tiles) {
      const { grids } = tile;
      const { layers } = grids;

      // 채널들을 순회하며 각각 다운로드
      for (const channel of Object.keys(layers) as Array<keyof GridLayers>) {
        // 1. 이미지 Blob 생성
        exportGridImage(layers[channel], channel).then((blob) => {
          // 2. 다운로드 실행
          if (blob) {
            triggerDownload(blob, `tile_${key}_${channel}.png`);
          }
        });
      }
    }
  };

  /**
   * 3. 병합된 이미지 다운로드 핸들러
   */
  const handleDownloadMergedImages = () => {
    for (const [key, tile] of tiles) {
      const { grids } = tile;
      const { layers } = grids;

      // 1. 합성 이미지 Blob 생성 (전체 채널 구조체 전달)
      exportMergedGridImage(layers).then((blob) => {
        // 2. 다운로드 실행
        if (blob) {
          triggerDownload(blob, `tile_${key}.png`);
        }
      });
    }
  };

  return (
    <div className="absolute bottom-20 left-6 flex flex-col gap-2 z-[100] pointer-events-auto select-none">
      <p className="text-[10px] text-white/50 font-mono mb-1 tracking-tighter">
        DEBUG: MULTI-CHANNEL ({tiles.size} Tiles)
      </p>

      <button
        onClick={handleDownloadAllBinary}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-blue-300 border border-blue-500/30 hover:bg-blue-500/20 active:scale-95',
        )}
      >
        Download All .BIN (x4)
      </button>

      <button
        onClick={handleDownloadAllImages}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/20 active:scale-95',
        )}
      >
        Preview Layers .PNG (x4)
      </button>

      {/* ✨ [New Button] Merged Preview */}
      <button
        onClick={handleDownloadMergedImages}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-amber-300 border border-amber-500/30 hover:bg-amber-500/20 active:scale-95',
        )}
      >
        Preview Merged .PNG (x1)
      </button>

      <div className="text-[9px] text-white/40 mt-1 px-1 font-mono leading-relaxed">
        Tiles Loaded: {tiles.size}
        <br />
        <span className="opacity-60">
          Keys: {Array.from(tiles.keys()).join(', ')}
        </span>
      </div>
    </div>
  );
}
