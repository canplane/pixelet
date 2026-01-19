// @pxl/web/modules/world/terrain/ui/export-tools.ts

import { cn } from "@/shared/lib/utils/cn.ts";

import { ivec } from "@pxl/spec/space/pvf3/mod.ts";

import { GROUND_CH_KEYS } from "@pxl/bake/terrain/format.ts";

import { useTileStore } from "@/modules/world/terrain/tile-store.ts";
import { triggerDownload } from "@/shared/lib/browser/download.ts";
// import { generateTileBinary } from "@pxl/bake/terrain/etc/tile-binary.ts";
import { export_ground_ch_image, export_ground_image } from "../export/exporter.ts";

/**
 * 활성화된 모든 월드 페이지의 '멀티 채널' 페이지 데이터를 추출합니다.
 */
export function WorldExportTools() {
  const tile_map = useTileStore((state) => state.tile_map);
  if (tile_map.size === 0) return null;

  // /**
  //  * 1. 바이너리 다운로드 핸들러
  //  */
  // const handle_download_all_binary = () => {
  //   for (const [page_idx, tile] of tile_map) {
  //     const { ground, ele } = tile.terrain;

  //     // (1) Elevation 채널 (Int16)
  //     const eleBin = generateTileBinary(ele);
  //     triggerDownload(eleBin, `tile_${ivec.repr(page_idx)}_elevation.bin`);

  //     // (2) Ground 채널들 (Uint8) - 병렬 실행
  //     GROUND_CH_KEYS.forEach((ch_key) => {
  //       const channelBin = generateTileBinary(ground[ch_key]);
  //       triggerDownload(channelBin, `tile_${ivec.repr(page_idx)}_${ch_key}.bin`);
  //     });
  //   }
  // };

  /**
   * 2. 채널별 이미지 프리뷰 핸들러 (Individual Channels)
   */
  const handle_download_all_images = () => {
    for (const [page_idx, tile] of tile_map) {
      const { ground } = tile.terrain;

      // 채널들을 순회하며 개별 이미지 생성
      GROUND_CH_KEYS.forEach((ch_key) => {
        // 🎯 [Refactor] exportTileImage의 'single' 모드 활용
        export_ground_ch_image(ground[ch_key], ch_key).then((blob) => {
          if (blob) {
            triggerDownload(blob, `tile_${ivec.repr(page_idx)}_${ch_key}.png`);
          }
        });
      });
    }
  };

  /**
   * 3. 병합된 이미지 다운로드 핸들러 (Merged Terrain)
   */
  const handle_download_merged_images = () => {
    for (const [page_idx, tile] of tile_map) {
      const { ground } = tile.terrain;

      // 🎯 [Refactor] exportTileImage의 'base' 모드 활용 (Painter's Algorithm 합성)
      export_ground_image(ground).then((blob) => {
        if (blob) {
          triggerDownload(blob, `tile_${ivec.repr(page_idx)}_merged.png`);
        }
      });
    }
  };

  // /**
  //  * 4. 고도 맵 이미지 다운로드 (Elevation Grayscale)
  //  */
  // const handleDownloadElevationImages = () => {
  //   for (const [pageId, page] of pageMap) {
  //     const { ground, ele } = page.base;

  //     // 🎯 elevation 쿼드트리를 직접 전달하여 모드 실행
  //     exportTileImage(ground, { mode: "elevation" }, undefined, ele).then((blob) => {
  //       if (blob) {
  //         triggerDownload(blob, `tile_${ivec.repr(pageId)}_elevation.png`);
  //       }
  //     });
  //   }
  // };

  return (
    <div className="absolute bottom-20 left-6 flex flex-col gap-2 z-100 pointer-events-auto select-none">
      <p className="text-[10px] text-white/50 font-mono mb-1 tracking-tighter">DEBUG: SPARSE-CHANNEL ({tile_map.size} Tiles)</p>

      {
        /* <button
        type="button"
        onClick={handleDownloadAllBinary}
        className={cn(
          "px-3 py-1.5 rounded-md text-[11px] font-bold transition-all",
          "bg-slate-800/80 text-blue-300 border border-blue-500/30 hover:bg-blue-500/20 active:scale-95",
        )}
      >
        Download All .BIN (x5)
      </button> */
      }

      <button
        type="button"
        onClick={handle_download_all_images}
        className={cn(
          "px-3 py-1.5 rounded-md text-[11px] font-bold transition-all",
          "bg-slate-800/80 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/20 active:scale-95",
        )}
      >
        Preview Channels .PNG (x4)
      </button>

      {/* ✨ [New Button] Merged Preview */}
      <button
        type="button"
        onClick={handle_download_merged_images}
        className={cn(
          "px-3 py-1.5 rounded-md text-[11px] font-bold transition-all",
          "bg-slate-800/80 text-amber-300 border border-amber-500/30 hover:bg-amber-500/20 active:scale-95",
        )}
      >
        Preview Merged .PNG (x1)
      </button>

      {
        /* <div className="text-[9px] text-white/40 mt-1 px-1 font-mono leading-relaxed">
        Tiles Loaded: {tile_map.size}
        <br />
        <span className="opacity-60">Keys: {Array.from(tile_map.keys()).join(", ")}</span>
      </div> */
      }
    </div>
  );
}
