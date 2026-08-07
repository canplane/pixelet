// @pxl/web/app/dev/dev_panel.tsx

import { LanguageToggleButton } from "./language_toggle_button.tsx";
import { useDebugStore } from "./debug_store.ts";

/** Debugging UI */
export function DevPanel() {
  const show_debug_objects = useDebugStore((state) => state.show_debug_objects);
  const toggle_debug_objects = useDebugStore.getState().toggle_debug_objects;

  const show_structures = useDebugStore((state) => state.show_structures);
  const toggle_structures = useDebugStore.getState().toggle_structures;

  return (
    // top-4 left-4 — swapped with top-right so the map's own address/sensor
    // status bar (top_overlays.tsx) can move to top-right, stacked above
    // the compass/scale_bar/GPS cluster that used to sit at bottom-right.
    <div className="fixed top-4 left-4 z-10 flex items-center gap-2">
      {/* multi-language toggle button */}
      {/* <LanguageToggleButton /> */}

      <button
        type="button"
        onClick={toggle_debug_objects}
        className={`px-2 py-1 rounded text-[10px] font-mono ${show_debug_objects ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
        title="frame-origin/anchor 디버그 오브젝트 표시 여부"
      >
        debug
      </button>

      {
        /* Only surfaces once debug mode itself is on — a second-tier toggle,
      not something that needs its own always-visible button. */
      }
      {show_debug_objects && (
        <button
          type="button"
          onClick={toggle_structures}
          className={`px-2 py-1 rounded text-[10px] font-mono ${show_structures ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="건물(구조물) 지형 채널 표시 여부"
        >
          건물
        </button>
      )}
    </div>
  );
}
