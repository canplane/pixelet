// @pxl/web/app/store/ui_store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { DEFAULT_OBJECT_SIZE } from "@/modules/world/objects/size.ts";

export type ActiveTool = "pointer" | "object" | "eraser";

export type ChatLine = { id: number; text: string };

// dummy_chat_text.tsx's own log cap — oldest line drops off once a new one
// would push the count past this, giving that display its "keeps rising
// and old lines fall off the top" behavior without any CSS scroll math.
const MAX_CHAT_LINES = 10;
let next_chat_line_id = 0;

type UiState = {
  /** Which map-canvas tap gesture currently means: "pointer" recenters the map, "object" places a cube, "eraser" removes one. */
  active_tool: ActiveTool;
  set_active_tool: (active_tool: ActiveTool) => void;

  object_color_id: ObjectColorId;
  set_object_color_id: (object_color_id: ObjectColorId) => void;

  /** One of objects/size.ts's OBJECT_SIZES — grid size the object/eraser tools place/erase at. */
  object_size: f64;
  set_object_size: (object_size: f64) => void;

  /** Sent-message log for top_overlays.tsx's dummy chat-text preview (dummy_chat_text.tsx) — appended only on an actual send (mode_bar.tsx's send button), not live per keystroke; mode_bar.tsx's own input value stays local component state. Still cosmetic-only (see mode_bar.tsx's own doc comment): nothing here reaches a real chat system yet. Capped to MAX_CHAT_LINES, oldest first. */
  chat_lines: ChatLine[];
  append_chat_line: (text: string) => void;
};

export const useUiStore = create<UiState>((set) => ({
  active_tool: "pointer",
  set_active_tool: (active_tool) => set({ active_tool }),

  object_color_id: "red",
  set_object_color_id: (object_color_id) => set({ object_color_id }),

  object_size: DEFAULT_OBJECT_SIZE,
  set_object_size: (object_size) => set({ object_size }),

  chat_lines: [],
  append_chat_line: (text) =>
    set((state) => ({
      chat_lines: [...state.chat_lines, { id: next_chat_line_id++, text }].slice(-MAX_CHAT_LINES),
    })),
}));
