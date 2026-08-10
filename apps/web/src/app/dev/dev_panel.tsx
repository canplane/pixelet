// @pxl/web/app/dev/dev_panel.tsx

import { useState } from "react";
import { LanguageToggleButton } from "./language_toggle_button.tsx";
import { useDebugStore } from "./debug_store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { Toast } from "@/shared/ui/toast/toast.tsx";
import { toast_custom, toast_dismiss } from "@/shared/ui/toast/toast_store.ts";

const DUMMY_TOAST_IDS = ["dummy-1", "dummy-2", "dummy-3", "dummy-4"];

/** Spawns 4 dummy toasts for testing the card-deck/swipe-to-rotate stack (one
 * more than toast.tsx's MAX_STACK_DEPTH+1 visible slots, to exercise the
 * beyond-visible/opacity-0 case) — a code-level trigger so it survives
 * reloads and doesn't need re-injecting via console every time. */
function spawn_dummy_toasts() {
  ["더미 토스트 1", "더미 토스트 2", "더미 토스트 3", "더미 토스트 4"].forEach((message, i) => {
    const id = DUMMY_TOAST_IDS[i];
    toast_custom(id, ({ index, stack_size, is_removing }) => <Toast id={id} index={index} stack_size={stack_size} is_removing={is_removing} message={message} />);
  });
}

function dismiss_dummy_toasts() {
  DUMMY_TOAST_IDS.forEach((id) => toast_dismiss(id));
}

const BUTTON_CLASS = "px-2 py-1 rounded text-[10px] font-mono text-left backdrop-blur-sm";

/** Debugging UI */
export function DevPanel() {
  const show_debug_menu = useDebugStore((state) => state.show_debug_menu);
  const toggle_debug_menu = useDebugStore.getState().toggle_debug_menu;

  const show_debug_objects = useDebugStore((state) => state.show_debug_objects);
  const toggle_debug_objects = useDebugStore.getState().toggle_debug_objects;

  const show_structures = useDebugStore((state) => state.show_structures);
  const toggle_structures = useDebugStore.getState().toggle_structures;

  const low_spec = useDebugStore((state) => state.low_spec);
  const toggle_low_spec = useDebugStore.getState().toggle_low_spec;

  const show_shadows = useDebugStore((state) => state.show_shadows);
  const toggle_show_shadows = useDebugStore.getState().toggle_show_shadows;

  // moved here from the main map overlay (see git history on
  // terrain_follow_toggle.tsx / map_overlays.tsx) — follow_terrain's
  // default is already true, so "terrain follows as you pan" reads as
  // normal/expected behavior, not something a regular user needs a visible
  // toggle for. The store state + toggle action were kept live the whole
  // time; only the always-visible button was pulled, so this just gives
  // that still-working toggle a debug-only way back in.
  const follow_terrain = useCameraStore((state) => state.follow_terrain);
  const toggle_follow_terrain = useCameraStore.getState().toggle_follow_terrain;

  const [dummy_toasts_shown, set_dummy_toasts_shown] = useState(false);
  function toggle_dummy_toasts() {
    if (dummy_toasts_shown) dismiss_dummy_toasts();
    else spawn_dummy_toasts();
    set_dummy_toasts_shown(!dummy_toasts_shown);
  }

  return (
    // top-4 left-4 — swapped with top-right so the map's own address/sensor
    // status bar (top_overlays.tsx) can move to top-right, stacked above
    // the compass/scale_bar/GPS cluster that used to sit at bottom-right.
    // flex-col items-start (not a row): once show_debug_menu started
    // revealing a growing list of secondary buttons, a row kept pushing
    // further right and eventually off-screen — a left-aligned column has
    // room to grow downward instead.
    <div className="fixed top-4 left-4 z-10 flex flex-col items-start gap-2">
      {/* multi-language toggle button */}
      {/* <LanguageToggleButton /> */}

      <button
        type="button"
        onClick={toggle_debug_menu}
        className={`${BUTTON_CLASS} ${show_debug_menu ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
        title="개발용 버튼 메뉴 표시 여부"
      >
        debug
      </button>

      {
        /* Everything below only surfaces once the menu itself is open —
        second-tier toggles, not things that need their own always-visible
        button. */
      }
      {show_debug_menu && (
        <button
          type="button"
          onClick={toggle_debug_objects}
          className={`${BUTTON_CLASS} ${show_debug_objects ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="frame-origin/anchor 디버그 오브젝트 표시 여부"
        >
          오브젝트
        </button>
      )}

      {show_debug_menu && (
        <button
          type="button"
          onClick={toggle_structures}
          className={`${BUTTON_CLASS} ${show_structures ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="건물(구조물) 지형 채널 표시 여부"
        >
          건물
        </button>
      )}

      {show_debug_menu && (
        <button
          type="button"
          onClick={toggle_follow_terrain}
          className={`${BUTTON_CLASS} ${follow_terrain ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="패닝 시 지형 고도를 따라갈지 여부"
        >
          지형 추적
        </button>
      )}

      {show_debug_menu && (
        // active (amber) = texture-pack mode ON, i.e. low_spec OFF — the
        // button reads "텍스처팩", so its highlighted state should mean
        // "texture-pack mode is active", not mirror the underlying
        // low_spec boolean directly.
        <button
          type="button"
          onClick={toggle_low_spec}
          className={`${BUTTON_CLASS} ${!low_spec ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="지형 색상 해상 모드 — 저사양(버텍스, 기본) / 텍스처팩(프래그먼트, 실제 타일링)"
        >
          텍스처팩
        </button>
      )}

      {show_debug_menu && (
        <button
          type="button"
          onClick={toggle_show_shadows}
          className={`${BUTTON_CLASS} ${show_shadows ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="태양/달 실시간 그림자맵 렌더 여부 — GPU 비용이 큰 편이라 성능 비교용"
        >
          그림자
        </button>
      )}

      {show_debug_menu && (
        <button
          type="button"
          onClick={toggle_dummy_toasts}
          className={`${BUTTON_CLASS} ${dummy_toasts_shown ? "bg-amber-400 text-black" : "bg-black/40 text-white/70"}`}
          title="카드덱/스와이프 회전 테스트용 더미 토스트 4개 생성/해제 토글(오버플로우 캡 테스트 포함)"
        >
          더미 토스트
        </button>
      )}
    </div>
  );
}
