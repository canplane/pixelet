// @pxl/web/locale/message-key.ts

/**
 * The locale key a kernel message reads as — `strerror` to the kernel's
 * `errno`.
 *
 * The kernel sends a number and never the words. This is the one place that
 * knows which words a number stands for, and it is here rather than beside
 * the enum because the words are the shell's.
 */

import { Message } from "@/kernel/abi/message.ts";

export function messageKey(message: Message): string | null {
  switch (message) {
    case Message.None:
      return null;

    case Message.MapLoading:
      return "toast.map_loading";
    case Message.MapLoadError:
      return "toast.map_load_error";
    case Message.CompanionLoading:
      return "toast.companion_loading";
    case Message.WorldReady:
      return "toast.world_ready";

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

    case Message.OrientationUnsupported:
      return "device.orientation.unsupported";
    case Message.OrientationPermissionError:
      return "device.orientation.permission_error";
    case Message.OrientationPermissionDenied:
      return "device.orientation.permission_denied";
    case Message.OrientationSignalLost:
      return "device.orientation.signal_lost";
  }
}
