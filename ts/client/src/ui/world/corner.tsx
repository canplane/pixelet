// @pxl/client/ui/world/corner.tsx

import { DebugWindow, useDebugWindow } from "@/ui/debug/window.tsx";
import { ChatLog } from "./chat-log.tsx";

export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugWindow />;
  return __IS_DEV__ && <ChatLog />;
}
