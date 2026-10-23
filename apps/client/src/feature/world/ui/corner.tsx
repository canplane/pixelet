// @pxl/client/feature/world/ui/corner.tsx

import { DebugPanels, useDebugWindow } from "@/feature/debug/panels.tsx";
import { ChatLog } from "./overlay/chat-log.tsx";

/** The window under the settings button: the debug panels while any is up, else the chat's lines. */
export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugPanels />;
  return __IS_DEV__ && <ChatLog />;
}
