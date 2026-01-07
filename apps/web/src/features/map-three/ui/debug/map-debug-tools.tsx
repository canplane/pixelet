import { useTileStore } from '@/entities/geo/model/tile-store';
import { cn } from '@/shared/lib/utils';
import { triggerDownload } from '@/shared/lib/io/browser-io';
import { generateTileBinary } from '@pixelet/core/entities/geo/lib/io/tile-binary';
import { exportTileImage } from '@/entities/geo/lib/export/tile-exporter';
import { TILE_SURFACE_KEYS } from '@pixelet/core/entities/geo/model/tiles';

/**
 * [map-debug-tools]
 * 활성화된 모든 월드 타일의 '멀티 채널' 타일 데이터를 추출합니다.
 */
export function MapDebugTools() {
  const tileMap = useTileStore((state) => state.tileMap);

  if (tileMap.size === 0) return null;

  /**
   * 1. 바이너리 다운로드 핸들러
   */
  const handleDownloadAllBinary = () => {
    for (const [key, tile] of tileMap) {
      const { ele, surface } = tile.base;

      // (1) Elevation 채널 (Int16)
      const eleBin = generateTileBinary(ele);
      triggerDownload(eleBin, `tile_${key}_elevation.bin`);

      // (2) Surface 채널들 (Uint8) - 병렬 실행
      TILE_SURFACE_KEYS.forEach((channelKey) => {
        const channelBin = generateTileBinary(surface[channelKey]);
        triggerDownload(channelBin, `tile_${key}_${channelKey}.bin`);
      });
    }
  };

  /**
   * 2. 채널별 이미지 프리뷰 핸들러 (Individual Channels)
   */
  const handleDownloadAllImages = () => {
    for (const [key, tile] of tileMap) {
      const { surface } = tile.base;

      // 채널들을 순회하며 개별 이미지 생성
      TILE_SURFACE_KEYS.forEach((channelKey) => {
        // 🎯 [Refactor] exportTileImage의 'single' 모드 활용
        exportTileImage(surface, { mode: 'single', targetChannel: channelKey }).then((blob) => {
          if (blob) {
            triggerDownload(blob, `tile_${key}_${channelKey}.png`);
          }
        });
      });
    }
  };

  /**
   * 3. 병합된 이미지 다운로드 핸들러 (Merged Terrain)
   */
  const handleDownloadMergedImages = () => {
    for (const [key, tile] of tileMap) {
      const { surface } = tile.base;

      // 🎯 [Refactor] exportTileImage의 'base' 모드 활용 (Painter's Algorithm 합성)
      exportTileImage(surface, { mode: 'base' }).then((blob) => {
        if (blob) {
          triggerDownload(blob, `tile_${key}_merged.png`);
        }
      });
    }
  };

  /**
   * 4. 고도 맵 이미지 다운로드 (Elevation Grayscale)
   */
  const handleDownloadElevationImages = () => {
    for (const [key, tile] of tileMap) {
      const { surface, ele } = tile.base;

      // 🎯 elevation 쿼드트리를 직접 전달하여 모드 실행
      exportTileImage(surface, { mode: 'elevation' }, undefined, ele).then((blob) => {
        if (blob) {
          triggerDownload(blob, `tile_${key}_elevation.png`);
        }
      });
    }
  };

  return (
    <div className="absolute bottom-20 left-6 flex flex-col gap-2 z-[100] pointer-events-auto select-none">
      <p className="text-[10px] text-white/50 font-mono mb-1 tracking-tighter">DEBUG: SPARSE-CHANNEL ({tileMap.size} Tiles)</p>

      <button
        onClick={handleDownloadAllBinary}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-blue-300 border border-blue-500/30 hover:bg-blue-500/20 active:scale-95',
        )}
      >
        Download All .BIN (x5)
      </button>

      <button
        onClick={handleDownloadAllImages}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/20 active:scale-95',
        )}
      >
        Preview Channels .PNG (x4)
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
        Tiles Loaded: {tileMap.size}
        <br />
        <span className="opacity-60">Keys: {Array.from(tileMap.keys()).join(', ')}</span>
      </div>
    </div>
  );
}
