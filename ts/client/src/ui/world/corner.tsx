// @pxl/client/ui/world/corner.tsx

import { DebugWindow, useDebugWindow } from "@/ui/debug/window.tsx";
import { ChatLog } from "./chat-log.tsx";

/** The window under the settings button. It shows the debug panels while any
 * is up, and the chat lines otherwise. */
export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugWindow />;
  return __IS_DEV__ && <ChatLog />;
}
