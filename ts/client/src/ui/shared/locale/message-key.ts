// @pxl/client/ui/shared/locale/message-key.ts

import {
  type Message,
  MESSAGE_WORLD_READ_FAILING,
  MESSAGE_WORLD_UNAVAILABLE,
} from "@/abi/overlay.enum.ts";
export function message_key(message: Message): string {
  switch (message) {
    case MESSAGE_WORLD_READ_FAILING:
      return "toast.server_unstable";
    case MESSAGE_WORLD_UNAVAILABLE:
      return "panic.server";
  }
}
