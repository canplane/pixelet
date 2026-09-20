// @pxl/web/feature/map/ui/overlay/dummy-chat-text.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useUiStore } from "@/app/store/ui-store.ts";

// Layered text-shadow, not -webkit-text-stroke: the stroke property renders
// inconsistently across engines and can clip thin glyph strokes at small sizes.
// 8 copies (4 diagonal + 4 cardinal), not just 4 diagonal — diagonal-only leaves
// gaps on vertical/horizontal strokes.
const TEXT_OUTLINE_SHADOW = "-1px 0 0 #000, 1px 0 0 #000, 0 -1px 0 #000, 0 1px 0 #000, -1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000, 1px 1px 0 #000, 2px 3px 4px rgba(0,0,0,0.6)";

export function DummyChatText() {
  const { t } = useTranslation();
  const chatLines = useUiStore((s) => s.chatLines);
  const [hovered, setHovered] = useState(false);

  if (chatLines.length === 0) return null;

  return (
    // pointer-events-auto against the map's default — needed for this group to receive hover.
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="pointer-events-auto max-w-[min(320px,calc(100vw-2rem))] flex flex-col items-start gap-1"
    >
      {chatLines.map((line, i) => (
        <div
          key={line.id}
          className="flex items-baseline gap-1.5 max-w-full transition-opacity duration-200"
          style={{ opacity: hovered ? 0.95 : 0.23 + 0.47 * ((i + 1) / chatLines.length) }}
        >
          <span className="text-orange-400 text-xs font-medium shrink-0" style={{ textShadow: TEXT_OUTLINE_SHADOW }}>
            {t("map.chat.self_label")}
          </span>
          <span className="text-white text-xs font-medium break-words" style={{ textShadow: TEXT_OUTLINE_SHADOW }}>
            {line.text}
          </span>
        </div>
      ))}
    </div>
  );
}
