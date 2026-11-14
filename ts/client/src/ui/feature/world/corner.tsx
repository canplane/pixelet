// @pxl/client/ui/feature/world/corner.tsx

import { DebugWindow, useDebugWindow } from "@/ui/feature/debug/window.tsx";
import { ChatLog } from "./chat-log.tsx";

export function Corner() {
  const debug = useDebugWindow();
  if (debug) return <DebugWindow />;
  return __DEV__ && <ChatLog />;
}
