// @pxl/client/ui/world/corner.tsx

import { DebugWindow, useDebugWindow } from "@/ui/debug/window.tsx";
import { ChatLog } from "./chat-log.tsx";

/** The window under the settings button: the debug panels while any is up, else the chat's lines. */
export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugWindow />;
  return __IS_DEV__ && <ChatLog />;
}
