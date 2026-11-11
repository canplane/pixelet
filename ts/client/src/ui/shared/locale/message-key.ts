// @pxl/client/ui/shared/locale/message-key.ts

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

    case Message.WorldReadFailing:
      return "toast.server_unstable";
    case Message.WorldUnavailable:
      return "panic.server";
  }
}
