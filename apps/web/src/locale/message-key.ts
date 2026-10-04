// @pxl/web/locale/message-key.ts

/** The locale key a kernel message number reads as: `strerror` to its `errno`. */

import { Message } from "@/kernel/abi/cmd.spec.ts";

export function messageKey(message: Message): string | null {
  switch (message) {
    case Message.None:
      return null;

    case Message.LocationChecking:
      return "toast.location_checking";
    case Message.LocationWeakSignal:
      return "toast.location_weak_signal";
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

    case Message.OrientationPermissionError:
      return "device.orientation.permission_error";
    case Message.OrientationPermissionDenied:
      return "device.orientation.permission_denied";
    case Message.OrientationSignalLost:
      return "device.orientation.signal_lost";
  }
}
