// @pxl/client/shell/locale/message-key.ts

/** The locale key a kernel message number reads as: `strerror` to its `errno`. */

import { Message } from "@/abi/cmd.spec.ts";

export function messageKey(message: Message): string {
  switch (message) {
    case Message.GeolocationUnsupported:
      return "device.geolocation.unsupported";
    case Message.GeolocationPermissionDenied:
      return "device.geolocation.permission_denied";
    case Message.GeolocationTimeout:
      return "device.geolocation.timeout";
    case Message.GeolocationPositionUnavailable:
    case Message.GeolocationGenericError:
      return "device.geolocation.generic_error";

    case Message.OrientationPermissionError:
      return "device.orientation.permission_error";
    case Message.OrientationPermissionDenied:
      return "device.orientation.permission_denied";
    case Message.OrientationSignalLost:
      return "device.orientation.signal_lost";

    case Message.WorldReadFailing:
      return "toast.server_unstable";
    case Message.WorldUnavailable:
      return "halt.server_failed";
  }
}
