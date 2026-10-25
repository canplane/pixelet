// @pxl/client/shell/world/corner.tsx

import { DebugPanels, useDebugWindow } from "@/shell/debug/panels.tsx";
import { ChatLog } from "./chat-log.tsx";

/** The window under the settings button: the debug panels while any is up, else the chat's lines. */
export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugPanels />;
  return __IS_DEV__ && <ChatLog />;
}
