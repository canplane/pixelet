import { useMapStore } from '../../model/map-store';
import { cn } from '@/shared/lib/utils';
import {
  downloadGridAsBinary,
  downloadGridAsImage,
} from '@/entities/osm/lib/tile-debug';

/**
 * [map-debug-tools]
 * 활성화된 모든 월드 타일의 그리드 데이터를 추출합니다.
 */
export function MapDebugTools() {
  const worldTiles = useMapStore((state) => state.worldTiles);

  if (worldTiles.size === 0) return null;

  const handleDownloadAllBinary = () => {
    worldTiles.forEach((tile) => {
      downloadGridAsBinary(
        tile.grid,
        tile.origin,
        tile.resolution,
        `tile_${tile.key}.bin`,
      );
    });
  };

  const handleDownloadAllImages = () => {
    worldTiles.forEach((tile) => {
      downloadGridAsImage(
        tile.grid,
        tile.resolution,
        tile.origin,
        `tile_${tile.key}.png`,
      );
    });
  };

  return (
    <div className="absolute bottom-20 left-6 flex flex-col gap-2 z-[100] pointer-events-auto select-none">
      <p className="text-[10px] text-white/50 font-mono mb-1 tracking-tighter">
        DEBUG: MULTI-TILE ({worldTiles.size})
      </p>

      <button
        onClick={handleDownloadAllBinary}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-blue-300 border border-blue-500/30 hover:bg-blue-500/20 active:scale-95',
        )}
      >
        Download All .BIN
      </button>

      <button
        onClick={handleDownloadAllImages}
        className={cn(
          'px-3 py-1.5 rounded-md text-[11px] font-bold transition-all',
          'bg-slate-800/80 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/20 active:scale-95',
        )}
      >
        Preview All .PNG
      </button>

      <div className="text-[9px] text-white/40 mt-1 px-1 font-mono leading-relaxed">
        Tiles Cached: {worldTiles.size}
        <br />
        <span className="opacity-60">
          Keys: {Array.from(worldTiles.keys()).join(', ')}
        </span>
      </div>
    </div>
  );
}
