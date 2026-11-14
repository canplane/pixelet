// @pxl/client/ui/shared/locale/message-key.ts

import {
  type Message,
  MESSAGE_GEOLOCATION_GENERIC_ERROR,
  MESSAGE_GEOLOCATION_PERMISSION_DENIED,
  MESSAGE_GEOLOCATION_POSITION_UNAVAILABLE,
  MESSAGE_GEOLOCATION_TIMEOUT,
  MESSAGE_GEOLOCATION_UNSUPPORTED,
  MESSAGE_WORLD_READ_FAILING,
  MESSAGE_WORLD_UNAVAILABLE,
} from "@/abi/overlay.enum.ts";

export function message_key(message: Message): string {
  switch (message) {
    case MESSAGE_GEOLOCATION_UNSUPPORTED:
      return "device.geolocation.unsupported";
    case MESSAGE_GEOLOCATION_PERMISSION_DENIED:
      return "device.geolocation.permission_denied";
    case MESSAGE_GEOLOCATION_TIMEOUT:
      return "device.geolocation.timeout";
    case MESSAGE_GEOLOCATION_POSITION_UNAVAILABLE:
    case MESSAGE_GEOLOCATION_GENERIC_ERROR:
      return "device.geolocation.generic_error";

    case MESSAGE_WORLD_READ_FAILING:
      return "toast.server_unstable";
    case MESSAGE_WORLD_UNAVAILABLE:
      return "panic.server";
  }
}
