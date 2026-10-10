// @pxl/web/locale/message-key.ts

/** The locale key a kernel message number reads as: `strerror` to its `errno`. */

import { Message } from "@/kernel/abi/cmd.spec.ts";

export function messageKey(message: Message): string {
  switch (message) {
    case Message.GeolocationUnsupported:
      return "device.geolocation.unsupported";
    case Message.GeolocationPermissionDenied:
      return "device.geolocation.permission_denied";
    case Message.GeolocationPositionUnavailable:
      return "device.geolocation.position_unavailable";
    case Message.GeolocationTimeout:
      return "device.geolocation.timeout";
    case Message.GeolocationGenericError:
      return "device.geolocation.generic_error";
    case Message.WorldGpuLost:
      return "load.gpu_lost";
    case Message.WorldOffline:
      return "load.offline";
    case Message.WorldReadFailing:
      return "load.read_failing";
  }
}
