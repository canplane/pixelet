// @pxl/web/modules/map/ui/overlay/dummy_chat_text.tsx

import { useState } from "react";
import { useUiStore } from "@/app/store/ui_store.ts";

// Layered text-shadow, not -webkit-text-stroke — the stroke property
// renders inconsistently across engines and can clip descenders/thin
// glyph strokes at small sizes. 8 solid 1px copies (4 diagonal + 4
// cardinal), not just the 4 diagonal ones — diagonal-only left visible
// gaps on purely vertical/horizontal glyph strokes (each diagonal shadow
// only contributes ~0.7px of coverage per axis at 1px offset, not a full
// 1px), which is exactly the "patchy" look the 4-corner version had. One
// more, larger/blurred + offset copy on top of all 8 gives the whole
// thing an actual drop shadow (depth), not just a flat outline.
const TEXT_OUTLINE_SHADOW =
  "-1px 0 0 #000, 1px 0 0 #000, 0 -1px 0 #000, 0 1px 0 #000, -1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000, 1px 1px 0 #000, 2px 3px 4px rgba(0,0,0,0.6)";

/**
 * Dummy chat-text log, top-left — each line is a past mode_bar.tsx send
 * (ui_store.ts's chat_lines), not a real chat message: there's no chat
 * system behind this yet (see ModeBar's own doc comment), just a
 * placeholder for where a Cyworld-style floating name/chat log would land
 * once one exists. Only appends on an actual send, not live per keystroke
 * (mode_bar.tsx's own input stays local state) — see ui_store.ts's
 * append_chat_line.
 *
 * Anchored top-left and capped at MAX_CHAT_LINES (ui_store.ts) rather than
 * scrolled: oldest line is simply dropped from the array once a new one
 * would push the count past the cap, so this component never needs its
 * own overflow/scroll handling. Plain static list, no enter/layout
 * animation — tried framer-motion here, dropped on request.
 */
export function DummyChatText() {
  const chat_lines = useUiStore((state) => state.chat_lines);
  // Hovering ANY line brightens the WHOLE log together, not just the one
  // under the cursor — a single group-level flag, not per-line state.
  const [hovered, set_hovered] = useState(false);

  if (chat_lines.length === 0) return null;

  return (
    // pointer-events-auto here (against the map's default) is what lets
    // this whole group answer mouse-enter/leave as one unit — individual
    // lines don't need their own handlers or their own pointer-events
    // override anymore.
    <div
      onMouseEnter={() => set_hovered(true)}
      onMouseLeave={() => set_hovered(false)}
      className="absolute top-4 left-4 z-50 pointer-events-auto max-w-[min(320px,calc(100vw-2rem))] flex flex-col items-start gap-1"
    >
      {chat_lines.map((line, i) => (
        // No background chip — bare outlined text, floating directly over
        // the map. Resting opacity still scales linearly by position:
        // oldest line reads faintest, newest still well short of solid —
        // a livestream chat overlay reads as unobtrusive precisely because
        // it never gets fully opaque at rest, even for the newest line.
        // transition-opacity is what makes the hover state (set on the
        // group above) a smooth fade instead of a hard snap.
        <div
          key={line.id}
          className="flex items-baseline gap-1.5 max-w-full transition-opacity duration-200"
          style={{ opacity: hovered ? 0.95 : 0.23 + 0.47 * ((i + 1) / chat_lines.length) }}
        >
          <span className="text-orange-400 text-xs font-medium shrink-0" style={{ textShadow: TEXT_OUTLINE_SHADOW }}>
            나
          </span>
          <span className="text-white text-xs font-medium break-words" style={{ textShadow: TEXT_OUTLINE_SHADOW }}>
            {line.text}
          </span>
        </div>
      ))}
    </div>
  );
}
