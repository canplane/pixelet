import { useMapStore } from '../../model/map-store';
import { cn } from '@/shared/lib/utils';
import {
  downloadGridAsBinary,
  downloadGridAsImage,
  downloadMergedGridAsImage, // ✨ Import 추가
} from '@/entities/osm/lib/debug/tile-debug';
import type { TileGrids } from '@/entities/osm/lib/baking/tile-baker';

/**
 * [map-debug-tools]
 * 활성화된 모든 월드 타일의 '멀티 채널' 그리드 데이터를 추출합니다.
 */
export function MapDebugTools() {
  const worldTiles = useMapStore((state) => state.worldTiles);

  if (worldTiles.size === 0) return null;

  const handleDownloadAllBinary = () => {
    worldTiles.forEach((tile) => {
      // 4개 채널을 순회하며 각각 다운로드
      (Object.keys(tile.grids) as Array<keyof TileGrids>).forEach((channel) => {
        const gridData = tile.grids[channel];
        // 파일명: tile_10:5_base.bin
        downloadGridAsBinary(
          gridData,
          tile.origin,
          tile.resolution,
          `tile_${tile.key}_${channel}.bin`,
        );
      });
    });
  };

  const handleDownloadAllImages = () => {
    worldTiles.forEach((tile) => {
      // 4개 채널을 순회하며 각각 이미지화
      (Object.keys(tile.grids) as Array<keyof TileGrids>).forEach((channel) => {
        const gridData = tile.grids[channel];
        // 파일명: tile_10:5_transport.png
        downloadGridAsImage(
          gridData,
          tile.resolution,
          tile.origin,
          `tile_${tile.key}_${channel}.png`,
        );
      });
    });
  };

  // ✨ [New] 병합된 이미지 다운로드 핸들러
  const handleDownloadMergedImages = () => {
    worldTiles.forEach((tile) => {
      downloadMergedGridAsImage(
        tile.grids, // 4개 채널 전체 구조체 전달
        tile.resolution,
        tile.origin,
        `tile_${tile.key}_merged.png`,
      );
    });
  };

  return (
    <div className="absolute bottom-20 left-6 flex flex-col gap-2 z-[100] pointer-events-auto select-none">
      <p className="text-[10px] text-white/50 font-mono mb-1 tracking-tighter">
        DEBUG: MULTI-CHANNEL ({worldTiles.size} Tiles)
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
        Tiles Loaded: {worldTiles.size}
        <br />
        <span className="opacity-60">
          Keys: {Array.from(worldTiles.keys()).join(', ')}
        </span>
      </div>
    </div>
  );
}
